#!/usr/bin/env python3
"""Unmodified airpods-tui IPC: u32 BE length + serde JSON AppEvent.
No TUI subprocesses, no fake battery, no unsupported Pro controls.
"""
import html
import json
import os
from pathlib import Path
import select
import signal
import socket
import struct
import sys
import time
import textwrap
import backend

class State:
    def __init__(self):
        self.sock = None
        self.buffer = b''
        self.batteries = {}
        self.ear = None
        self.lid = None
        self.product = None
        self.session = False
        self.initial = True
        self.connected_at = 0

    def connect(self):
        s = socket.socket(socket.AF_UNIX)
        s.settimeout(.3)
        try:
            s.connect(str(Path(os.environ['XDG_RUNTIME_DIR'])/'airpods-tui.sock'))
        except OSError:
            s.close()
            return False
        s.setblocking(False)
        self.sock=s
        self.buffer=b''
        self.connected_at=time.monotonic()
        self.initial=True
        return True

    def reset(self):
        if self.sock: self.sock.close()
        self.sock=None
        self.batteries={}
        self.ear=None
        self.lid=None
        self.session=False

    def event(self, data):
        address=backend.config()['airpods_address'].upper()
        if not isinstance(data,dict): return
        for kind in ('DeviceConnected','DeviceNearby'):
            item=data.get(kind)
            if isinstance(item,dict) and item.get('mac','').upper()==address:
                self.product=item.get('product_id')
                self.session=kind=='DeviceConnected'
        if str(data.get('DeviceDisconnected','')).upper()==address:
            self.batteries={}; self.ear=None; self.session=False
        pair=data.get('AACPEvent')
        if not isinstance(pair,list) or len(pair)!=2 or str(pair[0]).upper()!=address or not isinstance(pair[1],dict):
            return
        event=pair[1]
        for item in event.get('BatteryInfo',[]):
            component={4:'left',2:'right',8:'case'}.get(item.get('component'))
            level=item.get('level')
            if component and isinstance(level,int) and 0 <= level <= 100:
                if item.get('status')==4:
                    self.batteries.pop(component,None)
                else:
                    # Snapshot replay has no upstream timestamps: never call it fresh.
                    self.batteries[component]=(level, None if self.initial else time.monotonic())
        control=event.get('ControlCommand',{})
        if control.get('identifier')==10 and control.get('value') in ([1],[2]):
            self.ear=control['value']==[1]
        if 'CaseLid' in event:
            self.lid=event['CaseLid']

    def read(self):
        if not self.sock: return
        try:
            chunk=self.sock.recv(65536)
            if not chunk:
                self.reset(); return
            self.buffer+=chunk
            if len(self.buffer)>16*1024*1024+4:
                raise ValueError('IPC demasiado grande')
            while len(self.buffer)>=4:
                size=struct.unpack('!I',self.buffer[:4])[0]
                if size>16*1024*1024: raise ValueError('Trama IPC demasiado grande')
                if len(self.buffer)<size+4: break
                data=json.loads(self.buffer[4:4+size])
                self.buffer=self.buffer[4+size:]
                self.event(data)
        except BlockingIOError:
            pass
        except (OSError,ValueError,TypeError,KeyError):
            self.reset()

    def status(self):
        cfg=backend.config()
        try:
            bt=backend.bt_state()
            device=next((d for d in bt['devices'] if d['id'].upper()==cfg['airpods_address'].upper()),None)
        except Exception:
            device=None
        connected=bool(device and device['connected'])
        name=device['name'] if device else 'AirPods'
        modalias=device.get('modalias','') if device else ''
        product=self.product
        match=backend.re.search(r'p([0-9A-Fa-f]{4})',modalias)
        if product is None and match: product=int(match[1],16)
        models={0x2002:'AirPods · 1ª generación',0x200f:'AirPods · 2ª generación',0x2013:'AirPods · 3ª generación',0x2019:'AirPods · 4ª generación',0x201b:'AirPods 4 ANC',0x200e:'AirPods Pro',0x2014:'AirPods Pro 2',0x2024:'AirPods Pro · USB-C',0x2027:'AirPods Pro 3',0x200a:'AirPods Max',0x201f:'AirPods Max · 2024',0x202d:'AirPods Max 2'}
        model=models.get(product,'AirPods · Modelo no identificado')
        note=''
        result=dict(name=name,model=model,connected=connected,daemon=self.sock is not None,
            left='—',right='—',case='—',left_value=0,right_value=0,case_value=0,
            ear='No disponible',ear_available=False,lid='—',note=note,error='',battery='—',
            address=cfg['airpods_address'],fresh=False)
        known=[]
        for component,(level,stamp) in self.batteries.items():
            fresh=stamp is not None and time.monotonic()-stamp<=cfg['airpods_stale_seconds']
            if fresh and component!='case' and connected:
                result[component]=f'{level}%'
                result[component+'_value']=level
                if component!='case': known.append(level)
                result['fresh']=True
            elif self.sock:
                result[component]=f'{level}% · anterior'
                result[component+'_value']=level
        if connected and self.sock and self.ear is not None:
            result['ear']='Activada' if self.ear else 'Desactivada'
            result['ear_available']=True
        if self.lid: result['lid']={'Open':'Abierto','Closed':'Cerrado'}.get(self.lid,'—')
        if known: result['battery']=f'{min(known)}%'
        elif connected and device and device['battery']!='—':
            result['battery']=device['battery']
        if not self.sock:
            result['error']='Daemon no disponible. BlueZ sigue permitiendo conectar y reproducir.'
        else:
            try: apple='bluetooth:004C:' in Path('/etc/bluetooth/main.conf').read_text()
            except OSError: apple=False
            if not apple:
                result['error']='Apple DeviceID no configurado. AACP puede ser limitado; ver README.'
            elif not self.batteries:
                result['error']='Sin datos AACP. Abre el estuche o conecta los AirPods.'
            elif not result['fresh']:
                result['error']='Datos anteriores sin fecha de medición en IPC; no se consideran actuales.'
        result['note']='\n'.join(textwrap.wrap(result['note'],width=40))
        result['error']='\n'.join(textwrap.wrap(result['error'],width=40))
        return result

    def waybar(self):
        d=self.status()
        if not d['connected'] and backend.config()['airpods_hide_disconnected']:
            text=''
        else:
            text=f"󱡏 {d['battery']}" if d['connected'] else '󱡏 —'
        tooltip=f"{d['name']}\n{d['model']}\n{'Conectados' if d['connected'] else 'Desconectados'}\nIzquierdo: {d['left']} · Derecho: {d['right']}\nEstuche: {d['case']}\n{d['note']}\n{d['error']}"
        return dict(text=text,tooltip=html.escape(tooltip), **{'class':'connected' if d['connected'] else 'disconnected'})

def run(watch,waybar):
    state=State()
    stop=False
    def quit(*args):
        nonlocal stop
        stop=True
    signal.signal(signal.SIGTERM,quit)
    signal.signal(signal.SIGINT,quit)
    last=None
    next_connect=0
    next_output=0
    while not stop:
        now=time.monotonic()
        if not state.sock and now>=next_connect:
            state.connect()
            next_connect=now+5
        if state.sock:
            if select.select([state.sock],[],[],.2)[0]: state.read()
            if time.monotonic()-state.connected_at>1: state.initial=False
        else:
            time.sleep(.2)
        if now>=next_output:
            output=json.dumps(state.waybar() if waybar else state.status(),ensure_ascii=False)
            if output!=last:
                print(output,flush=True)
                last=output
            next_output=now+3
            if not watch: break
    state.reset()

if __name__=='__main__':
    run('--watch' in sys.argv, '--waybar' in sys.argv)
