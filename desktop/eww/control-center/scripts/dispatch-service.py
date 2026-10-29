#!/usr/bin/env python3
"""Session-local persistent dispatcher. Bounded slider mailbox, no shell commands from IPC."""
import collections
import importlib.util
import json
import os
from pathlib import Path
import queue
import signal
import socket
import struct
import threading
import time
import backend

# The mailbox already coalesces bursts. DDC remains physically rate-limited.
backend.SLIDER_DELAY = 0

spec=importlib.util.spec_from_file_location('popup_manager', Path(__file__).with_name('popup-manager.py'))
popup=importlib.util.module_from_spec(spec)
spec.loader.exec_module(popup)

class SliderMailbox:
    """One pending absolute target per control, with independent module workers."""
    def __init__(self, execute):
        self.execute=execute
        self.condition=threading.Condition()
        self.pending={}
        self.ready=collections.deque()
        self.active=set()
        self.running=True

    def submit(self, args):
        module, act, value=args
        key=(module,act)
        with self.condition:
            if act=='step' and key in self.pending:
                # Relative wheel events are accumulated, not replaced.
                value=str(float(self.pending[key][2])+float(value))
            self.pending[key]=(module,act,value)
            if key not in self.ready:
                self.ready.append(key)
            self.condition.notify()

    def worker(self):
        while True:
            with self.condition:
                self.condition.wait_for(lambda:not self.running or any(key[0] not in self.active for key in self.ready))
                if not self.running:return
                key=next(key for key in self.ready if key[0] not in self.active)
                self.ready.remove(key);self.active.add(key[0])
                args=self.pending.pop(key)
            try:
                self.execute(args)
            finally:
                with self.condition:
                    self.active.remove(key[0])
                    self.condition.notify_all()

    def stop(self):
        with self.condition:
            self.running=False;self.condition.notify_all()


def execute_action(args):
    module=args[0]
    try:
        # Accumulated wheel steps are split to retain backend's validated ±10 limit.
        if len(args)==3 and args[1]=='step':
            remaining=float(args[2])
            while remaining:
                step=max(-10,min(10,remaining));backend.action(module,'step',str(step));remaining-=step
        else:
            backend.action(*args)
    except Exception as e:
        backend.message(module,str(e))
        print(f'Action {module}: {e}',flush=True)
    finally:
        backend.notify_refresh(module)
        if module=='wifi' and len(args)>1 and args[1]=='connect':
            import subprocess
            subprocess.run(['systemctl','--user','stop','cc-wifi-agent.service'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)


def validate(request):
    if not isinstance(request,dict):raise ValueError('Invalid request')
    kind=request.get('kind');args=request.get('args')
    if not isinstance(args,list) or len(args)>3 or any(not isinstance(a,str) or len(a)>256 for a in args):
        raise ValueError('Invalid arguments')
    if kind=='popup':
        if len(args)>2 or (args and args[0] not in {*popup.WINDOWS,'close','outside','cleanup'}):raise ValueError('Invalid popup')
    elif kind=='action':
        if len(args)<2 or args[0] not in backend.DEFAULTS:raise ValueError('Invalid action')
        if args[1] in ('set','step','temperature'):
            if len(args)!=3:raise ValueError('Missing slider value')
            value=float(args[2])
            import math
            if not math.isfinite(value):raise ValueError('Invalid number')
            if args[1]=='temperature':
                if args[0]!='nightlight' or not 2500<=value<=6500:raise ValueError('Invalid temperature')
            elif args[1]=='step':
                if not -10<=value<=10:raise ValueError('Invalid step')
            elif not 0<=value<=100:raise ValueError('Invalid percent')
    else:raise ValueError('Invalid request kind')
    return kind,args


def main():
    # GTK belongs to this main thread. Pump events between popup requests.
    import gi
    gi.require_version('Gdk','3.0')
    from gi.repository import Gdk,GLib
    Gdk.init([])
    # Warm mapping before accepting clicks; safe to fail if session is not ready.
    try:
        monitors=json.loads(backend.command(['hyprctl','-j','monitors']))
        for m in monitors:popup.screen_index(monitors,m['name'])
    except Exception as e:print(f'Monitor warmup: {e}',flush=True)
    # Load GI once before starting action workers.
    backend.Gio.bus_get_sync(backend.Gio.BusType.SYSTEM,None)
    path=backend.RUN/'dispatch.sock'
    # systemd singleton; flock also protects against accidental manual duplicates.
    import fcntl
    lock=open(backend.RUN/'dispatch.lock','a')
    fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
    path.unlink(missing_ok=True)
    server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
    server.bind(str(path));path.chmod(0o600);server.listen(64);server.settimeout(.25)
    actions=queue.Queue(maxsize=64)
    mailbox=SliderMailbox(execute_action)
    workers=[threading.Thread(target=mailbox.worker,daemon=True) for _ in range(3)]
    def action_worker():
        while True:execute_action(actions.get())
    workers.append(threading.Thread(target=action_worker,daemon=True))
    for worker in workers:worker.start()
    running=True
    def stop(*args):
        nonlocal running
        running=False
    signal.signal(signal.SIGTERM,stop);signal.signal(signal.SIGINT,stop)
    try:
        while running:
            context=GLib.MainContext.default()
            while context.pending():context.iteration(False)
            try:conn,_=server.accept()
            except socket.timeout:continue
            with conn:
                conn.settimeout(.5)
                try:
                    uid=struct.unpack('3i',conn.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))[1]
                    if uid!=os.getuid():raise ValueError('Wrong peer UID')
                    data=b''
                    while b'\n' not in data and len(data)<=4096:
                        chunk=conn.recv(4096-len(data)+1)
                        if not chunk:break
                        data+=chunk
                    if len(data)>4096 or b'\n' not in data:raise ValueError('Invalid message size')
                    kind,args=validate(json.loads(data.split(b'\n',1)[0]))
                    if kind=='popup':
                        popup.main(args)
                    elif args[1] in ('set','step','temperature'):
                        mailbox.submit(tuple(args))
                    else:
                        actions.put_nowait(tuple(args))
                    conn.sendall(b'OK\n')
                except Exception as e:
                    try:conn.sendall(('ERROR '+str(e)+'\n').encode())
                    except OSError:pass
                    print(f'Dispatch: {e}',flush=True)
    finally:
        mailbox.stop();server.close();path.unlink(missing_ok=True);lock.close()

if __name__=='__main__':main()
