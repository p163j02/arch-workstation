#!/usr/bin/env python3
"""Native adapters. Only validated numeric IDs/MACs cross Eww's shell boundary."""
import fcntl
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
import textwrap
import shutil
class LazyGI:
    """CLI actions without DBus must not pay the GI import cost."""
    def __init__(self, name):
        self.name = name
        self.module = None

    def __getattr__(self, name):
        if self.module is None:
            import importlib
            self.module = importlib.import_module('gi.repository.' + self.name)
        return getattr(self.module, name)

Gio, GLib = LazyGI('Gio'), LazyGI('GLib')

ROOT = Path(__file__).resolve().parents[1]
RUN = Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}')) / 'waybar-control-center'
RUN.mkdir(mode=0o700, exist_ok=True)
MAC = re.compile(r'^[0-9A-Fa-f]{2}(?::[0-9A-Fa-f]{2}){5}$')
ENV = dict(os.environ, LC_ALL='C')
SYSTEM_BUS = None

def config():
    return json.loads((ROOT / 'settings.json').read_text())

def command(args, timeout=8):
    p = subprocess.run(args, capture_output=True, text=True, env=ENV, timeout=timeout)
    if p.returncode:
        raise RuntimeError((p.stderr or p.stdout or 'Operación fallida').strip()[-400:])
    return p.stdout.strip()

def dbus(dest, path, interface, method, params=None, timeout=8000):
    global SYSTEM_BUS
    if SYSTEM_BUS is None:
        SYSTEM_BUS = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
    return SYSTEM_BUS.call_sync(dest, path, interface, method, params, None, Gio.DBusCallFlags.NONE, timeout, None).unpack()

def bluez():
    return dbus('org.bluez', '/', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects')[0]

def adapter(objects):
    configured = config().get('bluetooth_adapter', '')
    choices = [p for p, v in objects.items() if 'org.bluez.Adapter1' in v]
    if configured:
        choices = [p for p in choices if p.rsplit('/', 1)[-1] == configured]
    if len(choices) > 1:
        raise RuntimeError('Varios adaptadores: configura bluetooth_adapter en settings.json')
    if not choices:
        raise RuntimeError('Sin adaptador Bluetooth')
    return choices[0]

def bt_state():
    objects = bluez()
    ap = adapter(objects)
    a = objects[ap]['org.bluez.Adapter1']
    devices = []
    for path, interfaces in objects.items():
        d = interfaces.get('org.bluez.Device1')
        if not d or not path.startswith(ap + '/'):
            continue
        address = d.get('Address', '')
        if not MAC.fullmatch(address):
            continue
        name = d.get('Alias', d.get('Name', address))
        battery = interfaces.get('org.bluez.Battery1', {}).get('Percentage')
        devices.append(dict(id=address, name=name, connected=d.get('Connected', False), paired=d.get('Paired', False),
            battery=f'{battery}%' if isinstance(battery, int) and 0 <= battery <= 100 else '—',
            airpods='airpods' in name.lower(), icon={'input-mouse':'󰍽', 'input-keyboard':'󰌌'}.get(d.get('Icon'), '󰋋'), modalias=d.get('Modalias', '')))
    devices.sort(key=lambda d: (not d['connected'], not d['paired'], d['name'].lower()))
    return dict(available=True, enabled=a.get('Powered', False), status=a.get('Alias', 'Bluetooth'), scanning=a.get('Discovering', False), devices=devices)

def volume_level():
    raw = command(['wpctl', 'get-volume', '@DEFAULT_AUDIO_SINK@'])
    match = re.search(r'Volume:\s*([0-9.]+)', raw)
    if not match:
        raise RuntimeError('No se pudo consultar la salida predeterminada')
    return round(min(100, float(match[1])*100)), 'MUTED' in raw

MONO_SINK = 'waybar_cc_mono'

def audio_sinks():
    return json.loads(command(['pactl', '-f', 'json', 'list', 'sinks']))

def mono_info(sinks):
    return next((s for s in sinks if s['name'] == MONO_SINK), None)

def move_audio_streams(target, owned_module=None):
    for stream in json.loads(command(['pactl', '-f', 'json', 'list', 'sink-inputs'])):
        if owned_module is not None and stream.get('owner_module') == owned_module:
            continue
        command(['pactl', 'move-sink-input', str(stream['index']), target])

def set_audio_mode(mode):
    if mode not in ('mono', 'stereo'):
        raise ValueError('Modo de audio no válido')
    sinks = audio_sinks()
    mono = mono_info(sinks)
    default = command(['pactl', 'get-default-sink'])
    if mode == 'mono':
        if mono and default == MONO_SINK:
            return
        if mono:
            command(['pactl', 'unload-module', str(mono['owner_module'])])
        percent, mute = volume_level()
        module = int(command(['pactl', 'load-module', 'module-remap-sink',
            'sink_name=' + MONO_SINK, 'master=' + default, 'channels=1',
            'channel_map=mono', 'master_channel_map=mono', 'remix=yes',
            'sink_properties=device.description=Audio_Mono']))
        try:
            command(['pactl', 'set-sink-volume', default, '100%'])
            command(['pactl', 'set-sink-mute', default, '0'])
            command(['pactl', 'set-sink-volume', MONO_SINK, f'{percent}%'])
            command(['pactl', 'set-sink-mute', MONO_SINK, '1' if mute else '0'])
            write_pending(RUN/'mono-master', default)
            command(['pactl', 'set-default-sink', MONO_SINK])
            move_audio_streams(MONO_SINK, module)
        except Exception:
            command(['pactl', 'set-default-sink', default])
            command(['pactl', 'set-sink-volume', default, f'{percent}%'])
            command(['pactl', 'set-sink-mute', default, '1' if mute else '0'])
            command(['pactl', 'unload-module', str(module)])
            raise
    elif mono:
        try:
            master = (RUN/'mono-master').read_text()
        except OSError:
            master = ''
        physical = [s for s in sinks if s['name'] != MONO_SINK]
        target = next((s['name'] for s in physical if s['name'] == master), None)
        if not target:
            target = next((s['name'] for s in physical if s['name'] == default), None)
        if not target and physical:
            target = physical[0]['name']
        if not target:
            raise RuntimeError('No hay una salida estéreo disponible')
        if default == MONO_SINK:
            volumes = mono.get('volume', {}).values()
            percent = max((round(v['value'] * 100 / 65536) for v in volumes), default=0)
            mute = mono.get('mute', False)
            command(['pactl', 'set-sink-volume', target, f'{percent}%'])
            command(['pactl', 'set-sink-mute', target, '1' if mute else '0'])
        command(['pactl', 'set-default-sink', target])
        move_audio_streams(target, mono['owner_module'])
        command(['pactl', 'unload-module', str(mono['owner_module'])])
        (RUN/'mono-master').unlink(missing_ok=True)

def volume_state():
    percent, mute = volume_level()
    default = command(['pactl', 'get-default-sink'])
    all_sinks = audio_sinks()
    mono = mono_info(all_sinks)
    is_mono = bool(mono and default == MONO_SINK)
    try:
        selected = (RUN/'mono-master').read_text() if is_mono else default
    except OSError:
        selected = default
    sinks = []
    for s in all_sinks:
        if s['name'] == MONO_SINK:
            continue
        props = s.get('properties', {})
        ident = str(props.get('object.id', ''))
        if not ident.isdecimal():
            continue
        icon = '󰋋' if 'bluez' in s['name'] else '󰍹' if 'hdmi' in s['name'] else '󰕾'
        sinks.append(dict(id=ident, name=s.get('description', s['name']), display=(s.get('description',s['name'])[:26]+'…') if len(s.get('description',s['name']))>27 else s.get('description',s['name']), selected=s['name']==selected, icon=icon))
    return dict(available=True, percent=percent, mute=mute, sinks=sinks, mono=is_mono)

def fields(line):
    """nmcli terse fields escape BOTH colon and backslash."""
    result, field, escaped = [], '', False
    for c in line:
        if escaped:
            field += c
            escaped = False
        elif c == '\\':
            escaped = True
        elif c == ':':
            result.append(field)
            field = ''
        else:
            field += c
    result.append(field)
    return result

def wifi_interface():
    devices = [fields(l) for l in command(['nmcli', '-t', '-f', 'DEVICE,TYPE,STATE', 'device']).splitlines()]
    choices = [d[0] for d in devices if len(d)==3 and d[1]=='wifi']
    configured = config().get('wifi_interface', '')
    if configured:
        choices = [d for d in choices if d == configured]
    if len(choices) > 1:
        raise RuntimeError('Varios adaptadores Wi-Fi: configura wifi_interface')
    if not choices:
        raise RuntimeError('No hay adaptador Wi-Fi')
    return choices[0]

def wifi_state():
    iface = wifi_interface()
    enabled = command(['nmcli', 'radio', 'wifi']) == 'enabled'
    networks = []
    for line in command(['nmcli', '-t', '-f', 'IN-USE,BSSID,SSID,SIGNAL,SECURITY', 'device', 'wifi', 'list', 'ifname', iface, '--rescan', 'no']).splitlines():
        f = fields(line)
        if len(f)!=5 or not MAC.fullmatch(f[1]) or not f[3].isdigit() or not f[2]:
            continue
        networks.append(dict(id=f[1], name=f[2], signal=min(100,int(f[3])), secure=f[4] not in ('', '--'), connected=f[0]=='*'))
    networks.sort(key=lambda n: (not n['connected'], -n['signal'], n['name']))
    # Keep strongest AP per SSID/security, including active AP.
    unique, seen = [], set()
    for n in networks:
        key = (n['name'], n['secure'])
        if key not in seen:
            unique.append(n)
            seen.add(key)
    active = next((n for n in unique if n['connected']), None)
    status = command(['nmcli', '-g', 'GENERAL.STATE', 'device', 'show', iface])
    return dict(available=True, enabled=enabled, ssid=active['name'] if active else 'Sin conexión', signal=active['signal'] if active else 0, status='Conectado' if active else ('Wi-Fi desactivado' if not enabled else status), networks=unique)

def backlight():
    devices = sorted(Path('/sys/class/backlight').glob('*'))
    preferred = config().get('backlight_device', 'amdgpu_bl1')
    choices = [p for p in devices if p.name == preferred]
    if not choices and len(devices)==1:
        choices = devices
    if not choices:
        raise RuntimeError('Sin backlight compatible o selección ambigua; configura backlight_device')
    return choices[0], [p.name for p in devices]

def brightness_state():
    p, devices = backlight()
    maximum = int((p/'max_brightness').read_text())
    value = int((p/'brightness').read_text())
    external=external_brightness()
    return dict(available=True, percent=round(value*100/maximum), device=p.name, devices=devices,
        external=external, message='Brillo físico · notebook y monitor DDC/CI')

def external_brightness(force=False):
    connector=config().get('external_connector','HDMI-A-1')
    cache=RUN/'ddc-state.json'
    try:
        saved=json.loads(cache.read_text())
        if not force and time.time()-saved['time']<15: return saved['state']
    except (OSError,ValueError,KeyError): pass
    state=dict(available=False,percent=0,bus='',name=connector,message='Monitor externo: pendiente DDC/CI')
    try:
        connections=[p for p in Path('/sys/class/drm').glob('card*-'+connector) if (p/'status').read_text().strip()=='connected']
        if len(connections)!=1: raise RuntimeError('Monitor externo no conectado')
        bus=(connections[0]/'ddc').resolve().name
        if not re.fullmatch(r'i2c-\d+',bus): raise RuntimeError('Sin bus DDC identificable')
        state['bus']=bus[4:]
        if not shutil.which('ddcutil'): raise RuntimeError('Falta ddcutil y acceso I²C; ver README')
        if not os.access('/dev/'+bus,os.R_OK|os.W_OK): raise RuntimeError('Sin acceso a /dev/'+bus+'; ver README')
        raw=command(['ddcutil','--bus',state['bus'],'--brief','getvcp','10'],timeout=5)
        match=re.search(r'VCP\s+10\s+C\s+(\d+)\s+(\d+)',raw)
        if not match or int(match[2])<=0: raise RuntimeError('El monitor no respondió a brillo DDC/CI')
        state.update(available=True,percent=round(int(match[1])*100/int(match[2])),maximum=int(match[2]),message='DDC/CI conectado')
    except Exception as e: state['message']=str(e)
    cache.write_text(json.dumps(dict(time=time.time(),state=state)))
    return state

def set_displays(target):
    p,_=backlight()
    ext=external_brightness()
    if ext['available']:
        command(['ddcutil','--bus',ext['bus'],'setvcp','10',str(round(target*ext['maximum']/100))],timeout=5)
        # Confirmed write: retain the bus/maximum and avoid a getvcp round-trip
        # on every drag event. Periodic reads still reconcile hardware changes.
        ext = dict(ext, percent=round(target*ext['maximum']/100)*100/ext['maximum'])
        ext['percent'] = round(ext['percent'])
        cache = RUN/'ddc-state.json'
        tmp = cache.with_suffix(f'.{os.getpid()}.tmp')
        tmp.write_text(json.dumps(dict(time=time.time(), state=ext)))
        tmp.replace(cache)
    command(['brightnessctl','-d',p.name,'set',f'{target}%'])
    if not ext['available']:
        message('brightness','Notebook actualizado. Monitor externo: '+ext['message'])

def profiles_state():
    props=dbus('net.hadess.PowerProfiles','/net/hadess/PowerProfiles','org.freedesktop.DBus.Properties','GetAll',GLib.Variant('(s)',('net.hadess.PowerProfiles',)))[0]
    labels={'power-saver':('󰌪','Ahorro de energía','Menor consumo y rendimiento'), 'balanced':('󰾅','Equilibrado','Balance entre autonomía y velocidad'), 'performance':('󰓅','Rendimiento','Prioriza velocidad; mayor consumo')}
    items=[]
    for p in props['Profiles']:
        ident=p['Profile']
        if ident not in labels: continue
        icon,name,description=labels[ident]
        items.append(dict(id=ident,icon=icon,name=name,description=description,selected=props['ActiveProfile']==ident))
    return dict(available=True,profiles=items,degraded=props.get('PerformanceDegraded',''),active=props['ActiveProfile'],**battery_state())

def battery_duration(minutes):
    hours, remainder=divmod(minutes,60)
    return f'{hours} h {remainder} min' if hours else f'{remainder} min'

def battery_state():
    paths=dbus('org.freedesktop.UPower','/org/freedesktop/UPower','org.freedesktop.UPower','EnumerateDevices')[0]
    batteries=[]
    for path in paths:
        p=dbus('org.freedesktop.UPower',path,'org.freedesktop.DBus.Properties','GetAll',GLib.Variant('(s)',('org.freedesktop.UPower.Device',)))[0]
        if p.get('Type')==2 and p.get('PowerSupply') and p.get('IsPresent'): batteries.append(p)
    if len(batteries)!=1: return dict(percent=0,battery_available=False,battery_status='Batería no disponible',remaining='Sin estimación')
    p=batteries[0]; remaining=round(p.get('TimeToEmpty',0)/60); state=p.get('State',0)
    labels={1:'Cargando',2:'Descargando',3:'Vacía',4:'Carga completa',5:'Carga pendiente',6:'Descarga pendiente'}
    return dict(percent=round(p['Percentage']),battery_available=True,battery_status=labels.get(state,'Estado desconocido'),
        remaining=f'≈ {battery_duration(remaining)} restantes' if state==2 and remaining>0 else ('Conectado a corriente' if state in (1,4,5) else 'Calculando autonomía…'))

def nightlight_state():
    running=subprocess.run(['pgrep','-x','hyprsunset'],stdout=subprocess.DEVNULL).returncode==0
    if not running:
        return dict(available=True,running=False,percent=0,gamma=100,temperature=6500,warm=False)
    temperature=int(command(['hyprctl','hyprsunset','temperature']))
    gamma=float(command(['hyprctl','hyprsunset','gamma']))
    return dict(available=True,running=True,percent=round(max(0,min(80,100-gamma))),gamma=gamma,temperature=temperature,warm=temperature<6000)

def ensure_sunset():
    if subprocess.run(['pgrep','-x','hyprsunset'],stdout=subprocess.DEVNULL).returncode:
        command(['systemctl','--user','start','hyprsunset.service'])
        for _ in range(20):
            try:
                command(['hyprctl','hyprsunset','temperature','6500'])
                return
            except RuntimeError: time.sleep(.1)
        raise RuntimeError('hyprsunset no respondió. Revisa su servicio de usuario.')

POWER_ACTIONS={'lock':('Bloquear','hyprlock'), 'shutdown':('Apagar','systemctl'), 'logout':('Cerrar sesión','hyprctl'), 'reboot':('Reiniciar','systemctl')}

def power_state():
    try:
        pending=json.loads((RUN/'power-confirm.json').read_text())
        action=pending['action'] if time.time()-pending['time']<20 else ''
    except (OSError,ValueError,KeyError): action=''
    return dict(available=True,pending=action,confirmation=f'¿{POWER_ACTIONS[action][0]}?' if action in POWER_ACTIONS else '')

DEFAULTS = {
    'profiles':dict(available=False,profiles=[],active='',degraded='',percent=0,battery_available=False,battery_status='No disponible',remaining='Sin estimación'),
    'nightlight':dict(available=False,running=False,percent=0,gamma=100,temperature=6500,warm=False),
    'power':dict(available=False,pending='',confirmation=''),
    'volume':dict(available=False,percent=0,mute=False,sinks=[],mono=False),
    'wifi':dict(available=False,enabled=False,ssid='Sin conexión',status='No disponible',signal=0,networks=[]),
    'brightness':dict(available=False,percent=0,device='—',devices=[],message='No disponible',external=dict(available=False,percent=0,message='No disponible')),
    'bluetooth':dict(available=False,enabled=False,status='No disponible',scanning=False,devices=[])}

def status(module):
    try:
        state = globals()[module+'_state']() if module!='bluetooth' else bt_state()
        state['error'] = ''
    except Exception as e:
        state = dict(DEFAULTS[module], error=str(e))
    msg = RUN/(module+'-message.json')
    try:
        m = json.loads(msg.read_text())
        if time.time()-m['time'] < (60 if m['busy'] else 30):
            state['error'] = m['text']
            state['busy'] = m['busy']
    except (OSError, ValueError, KeyError):
        pass
    state.setdefault('busy', False)
    if module=='wifi':
        state['spinner']='󰑓'
    state['error']='\n'.join(textwrap.wrap(state['error'],width=38))
    return state

def message(module, text, busy=False):
    path = RUN/(module+'-message.json')
    tmp = path.with_suffix(f'.{os.getpid()}.tmp')
    tmp.write_text(json.dumps(dict(time=time.time(),text=text,busy=busy)))
    tmp.replace(path)

def ensure_agent(name):
    # Lifecycle owned by systemd, not orphaned children; started only on demand.
    binary = {'wifi':'nm-applet','bluetooth':'blueman-applet'}[name]
    check = subprocess.run(['pgrep','-x',binary], stdout=subprocess.DEVNULL)
    if check.returncode:
        command(['systemd-run','--user',f'--unit=cc-{name}-agent','--collect',binary])
        time.sleep(1)

def numeric(value):
    number = float(value)
    if not 0 <= number <= 100:
        raise ValueError('Porcentaje fuera de rango')
    return round(number)

def notify_refresh(module):
    """Atomic completion event consumed by active Eww listeners, not a fake state."""
    path = RUN/(module+'-refresh')
    tmp = path.with_suffix(f'.{os.getpid()}.tmp')
    tmp.write_text(str(time.monotonic_ns()))
    tmp.replace(path)

def write_pending(path, value):
    tmp = path.with_suffix(f'.{os.getpid()}.tmp')
    tmp.write_text(str(value))
    tmp.replace(path)

def action(module, act, value=''):
    with open(RUN/(module+'-action.lock'), 'a') as lock:
        return locked_action(module, act, value, lock)

# CLI fallback coalesces; the persistent service has its own bounded mailbox.
SLIDER_DELAY = .04
DDC_DELAY = .12

def locked_action(module, act, value, lock):
    if module=='nightlight' and act=='temperature':
        kelvin=float(value)
        if not 2500<=kelvin<=6500: raise ValueError('Temperatura fuera de rango (2500–6500 K)')
        pending=RUN/'kelvin-pending'; write_pending(pending, round(kelvin))
        try: fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        except BlockingIOError: return
        consumed = None
        ensure_sunset()
        while consumed != pending.stat().st_mtime_ns:
            time.sleep(SLIDER_DELAY)
            consumed = pending.stat().st_mtime_ns
            temperature = pending.read_text()
            command(['hyprctl','hyprsunset','temperature',temperature])
            notify_refresh(module)
        return
    if act in ('set', 'step'):
        # Latest absolute target wins; DDC stays limited, local controls use 40 ms.
        target = numeric(value) if act=='set' else float(value)
        if act=='step' and not -10 <= target <= 10:
            raise ValueError('Incremento fuera de rango')
        pending = RUN/(module+'-pending')
        write_pending(pending, target)
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return
        time.sleep(DDC_DELAY if module=='brightness' else SLIDER_DELAY)
        consumed = pending.stat().st_mtime_ns
        target = float(pending.read_text())
        if module=='volume':
            current = volume_level()[0]
            target = min(100,max(0,current+target)) if act=='step' else target
            if round(target)!=current:
                command(['wpctl','set-volume','-l','1','@DEFAULT_AUDIO_SINK@',f'{target}%'])
        elif module=='brightness':
            state = brightness_state()
            target = state['percent']+target if act=='step' else target
            target = min(100,max(config().get('brightness_min',5),target))
            ext=state['external']
            if round(target)!=state['percent'] or (ext['available'] and round(target)!=ext['percent']):
                set_displays(target)
        elif module=='nightlight':
            ensure_sunset()
            current=nightlight_state()['percent']
            target=min(80,max(0,current+target if act=='step' else target))
            command(['hyprctl','hyprsunset','gamma',str(100-target)])
        else:
            raise ValueError('Este módulo no admite porcentajes')
        notify_refresh(module)
        # If a newer absolute target arrived during the write, apply it before exit.
        while act=='set' and pending.stat().st_mtime_ns!=consumed:
            time.sleep(DDC_DELAY if module=='brightness' else SLIDER_DELAY)
            consumed = pending.stat().st_mtime_ns
            latest=numeric(pending.read_text())
            if module=='volume':
                command(['wpctl','set-volume','-l','1','@DEFAULT_AUDIO_SINK@',f'{latest}%'])
            elif module=='brightness' and state['available']:
                latest=max(config().get('brightness_min',5),latest)
                set_displays(latest)
            elif module=='nightlight':
                command(['hyprctl','hyprsunset','gamma',str(100-min(80,latest))])
            notify_refresh(module)
        return
    fcntl.flock(lock, fcntl.LOCK_EX)
    message(module, 'Operación en curso…', True)
    if module=='profiles':
        if act!='profile' or value not in [p['id'] for p in profiles_state()['profiles']]:
            raise ValueError('Perfil no disponible')
        command(['powerprofilesctl','set',value])
        message(module,'Perfil cambiado. La autonomía se recalcula con el consumo real.')
        return
    elif module=='nightlight':
        if act!='warm': raise ValueError('Acción no válida')
        ensure_sunset()
        temperature=6500 if nightlight_state()['warm'] else 3500
        command(['hyprctl','hyprsunset','temperature',str(temperature)])
    elif module=='power':
        confirmation=RUN/'power-confirm.json'
        if act=='cancel':
            confirmation.unlink(missing_ok=True)
        elif act=='request' and value in POWER_ACTIONS:
            if value=='lock':
                if subprocess.run(['pgrep','-x','hyprlock'],stdout=subprocess.DEVNULL).returncode:
                    command(['systemd-run','--user','--unit=cc-screen-lock','--collect','hyprlock'])
                command([str(ROOT/'scripts/popup-manager.py'),'close'])
            else:
                confirmation.write_text(json.dumps(dict(action=value,time=time.time())))
        elif act=='confirm' and value in POWER_ACTIONS and power_state()['pending']==value:
            confirmation.unlink(missing_ok=True)
            command([str(ROOT/'scripts/popup-manager.py'),'close'])
            command({'shutdown':['systemctl','poweroff'],'reboot':['systemctl','reboot'],'logout':['hyprctl','dispatch','hl.dsp.exit()']}[value])
        else:
            raise ValueError('Confirmación vencida o acción no válida')
    elif module=='volume':
        if act=='mute':
            command(['wpctl','set-mute','@DEFAULT_AUDIO_SINK@','toggle'])
        elif act=='mode':
            set_audio_mode(value)
        elif act=='sink' and value.isdecimal() and any(s['id']==value for s in volume_state()['sinks']):
            was_mono = volume_state()['mono']
            if was_mono:
                set_audio_mode('stereo')
            command(['wpctl','set-default',value])
            if was_mono:
                set_audio_mode('mono')
        else:
            raise ValueError('Salida o acción no válida')
    elif module=='wifi':
        iface = wifi_interface()
        if act=='toggle':
            command(['nmcli','radio','wifi','off' if wifi_state()['enabled'] else 'on'])
        elif act=='rescan':
            message(module, 'Buscando redes…', True)
            path=command(['nmcli','-g','GENERAL.DBUS-PATH','device','show',iface])
            def last_scan():
                return dbus('org.freedesktop.NetworkManager',path,'org.freedesktop.DBus.Properties','Get',GLib.Variant('(ss)',('org.freedesktop.NetworkManager.Device.Wireless','LastScan')))[0]
            before=last_scan()
            started=time.monotonic()
            command(['nmcli','device','wifi','rescan','ifname',iface], 20)
            # nmcli only requests the scan. Wait for NetworkManager's completion timestamp.
            while last_scan()<=before:
                if time.monotonic()-started>20:
                    raise RuntimeError('La búsqueda tardó demasiado. Vuelve a intentar.')
                time.sleep(.2)
            time.sleep(max(0,1.2-(time.monotonic()-started)))
            message(module, 'Lista de redes actualizada.')
            return
        elif act=='disconnect':
            command(['nmcli','device','disconnect',iface],20)
        elif act=='connect' and MAC.fullmatch(value):
            networks = wifi_state()['networks']
            network = next((n for n in networks if n['id'].upper()==value.upper()), None)
            if not network:
                raise RuntimeError('La red ya no está disponible. Vuelve a escanear.')
            ensure_agent('wifi')
            # UUID lookup avoids SSID shell interpolation and duplicate profile names.
            profiles = command(['nmcli','-t','-f','UUID,TYPE','connection','show']).splitlines()
            saved = None
            for line in profiles:
                f = fields(line)
                if len(f)==2 and f[1]=='802-11-wireless' and re.fullmatch(r'[0-9a-fA-F-]{36}', f[0]):
                    ssid = command(['nmcli','-g','802-11-wireless.ssid','connection','show','uuid',f[0]])
                    if ssid==network['name']:
                        bound=command(['nmcli','-g','connection.interface-name','connection','show','uuid',f[0]])
                        if bound not in ('', '--', iface):
                            continue
                        saved = f[0]
                        break
            if saved:
                command(['nmcli','--wait','45','connection','up','uuid',saved,'ifname',iface,'ap',value], 50)
            else:
                # Secrets requested by registered NetworkManager agent; never arguments/stdin/logs.
                command(['nmcli','--wait','45','device','wifi','connect',value,'ifname',iface],50)
        else:
            raise ValueError('Acción Wi-Fi no válida')
    elif module=='bluetooth':
        objects = bluez()
        ap = adapter(objects)
        if act=='toggle':
            enabled = not objects[ap]['org.bluez.Adapter1'].get('Powered',False)
            dbus('org.bluez',ap,'org.freedesktop.DBus.Properties','Set',GLib.Variant('(ssv)',('org.bluez.Adapter1','Powered',GLib.Variant('b',enabled))))
        elif act=='scan':
            active = RUN/'scan-owned'
            if active.exists():
                active.unlink(missing_ok=True)
            else:
                active.touch(mode=0o600)
                try:
                    dbus('org.bluez',ap,'org.bluez.Adapter1','StartDiscovery')
                    until = time.monotonic()+20
                    while active.exists() and time.monotonic()<until:
                        time.sleep(.25)
                finally:
                    active.unlink(missing_ok=True)
                    dbus('org.bluez',ap,'org.bluez.Adapter1','StopDiscovery')
        elif act in ('connect','disconnect','pair') and MAC.fullmatch(value):
            path = next((p for p,v in objects.items() if v.get('org.bluez.Device1',{}).get('Address','').upper()==value.upper() and p.startswith(ap+'/')),None)
            if not path:
                raise RuntimeError('Dispositivo no encontrado')
            if act=='pair':
                ensure_agent('bluetooth')
            dbus('org.bluez',path,'org.bluez.Device1',{'connect':'Connect','disconnect':'Disconnect','pair':'Pair'}[act], timeout=45000)
            if act=='pair':
                message(module,'Emparejado. Pulsa Conectar para usarlo.')
        else:
            raise ValueError('Acción Bluetooth no válida')
    message(module, '')

def watch(module):
    """Eww stops this listener when its window closes. Event-driven with cheap recovery timer."""
    loop = GLib.MainLoop()
    last, dirty, heartbeat = None, True, 0
    scheduled = None
    def refresh():
        nonlocal last, dirty, heartbeat
        heartbeat += 1
        # sysfs is cheap; DBus event or 15s recovery refresh for other states.
        if dirty or module in ('brightness','power') or (module in ('nightlight','profiles') and heartbeat%2==0) or heartbeat%15==0:
            data = status(module)
            text = json.dumps(data, ensure_ascii=False)
            if text!=last:
                print(text,flush=True)
                last=text
            dirty=False
        else:
            # Message changes are local, not repeated expensive enumeration.
            p = RUN/(module+'-message.json')
            try:
                if time.time()-p.stat().st_mtime < 2:
                    dirty=True
            except OSError:
                pass
        return True
    # Cheap local feedback tick: render spinner immediately without enumerating networks.
    message_stamp=None
    frame=0
    def feedback():
        nonlocal last,dirty,message_stamp,frame
        if module!='wifi' or last is None:
            return True
        p=RUN/(module+'-message.json')
        try:
            stamp=p.stat().st_mtime_ns
            m=json.loads(p.read_text())
        except (OSError,ValueError):
            return True
        data=json.loads(last)
        busy=m.get('busy',False) and time.time()-m['time']<60
        if stamp!=message_stamp or busy or data.get('busy',False):
            if stamp!=message_stamp:
                dirty=True
            message_stamp=stamp
            data['busy']=busy
            data['error']='\n'.join(textwrap.wrap(m['text'],width=38)) if time.time()-m['time']<30 or busy else ''
            data['spinner']=['◐','◓','◑','◒'][frame%4] if busy else '󰑓'
            frame+=1
            text=json.dumps(data,ensure_ascii=False)
            if text!=last:
                print(text,flush=True); last=text
        return True
    def changed(*args):
        nonlocal dirty, scheduled
        dirty=True
        if scheduled is None:
            # Group event bursts, but do not wait for the 1-second recovery tick.
            scheduled = GLib.timeout_add(40, event_refresh)

    def event_refresh():
        nonlocal scheduled
        scheduled = None
        refresh()
        return False

    def local_changed(monitor, file, other, event):
        names = {file.get_basename(), other.get_basename() if other else None}
        if names & {module+'-refresh', module+'-message.json'}:
            changed()

    local_monitor = Gio.File.new_for_path(str(RUN)).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, None)
    local_monitor.connect('changed', local_changed)
    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM,None)
    sender = {'bluetooth':'org.bluez','wifi':'org.freedesktop.NetworkManager','profiles':'net.hadess.PowerProfiles'}.get(module)
    sub = bus.signal_subscribe(sender,None,None,None,None,Gio.DBusSignalFlags.NONE,changed,None) if sender else None
    proc = None
    if module=='volume':
        proc = subprocess.Popen(['pactl','subscribe'],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,env=ENV)
        def pulse(fd,condition):
            line=proc.stdout.readline()
            changed()
            return bool(line)
        GLib.io_add_watch(proc.stdout, GLib.IO_IN | GLib.IO_HUP,pulse)
    signal.signal(signal.SIGTERM,lambda *a: loop.quit())
    signal.signal(signal.SIGINT,lambda *a: loop.quit())
    refresh()
    GLib.timeout_add(1000,refresh)
    if module=='wifi': GLib.timeout_add(120,feedback)
    try:
        loop.run()
    finally:
        local_monitor.cancel()
        if scheduled is not None:
            GLib.source_remove(scheduled)
        if proc:
            proc.terminate()
            try: proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill(); proc.wait()
        if sub: bus.signal_unsubscribe(sub)

if __name__=='__main__':
    mode, module, *args = sys.argv[1:]
    if module not in DEFAULTS:
        sys.exit('Módulo inválido')
    if mode=='status':
        print(json.dumps(status(module),ensure_ascii=False))
    elif mode=='watch':
        watch(module)
    elif mode=='action':
        try:
            # Stop scanning must not wait for the scanner's action lock.
            if module=='bluetooth' and args[0]=='scan' and (RUN/'scan-owned').exists():
                (RUN/'scan-owned').unlink(missing_ok=True)
            else:
                action(module,*args)
        except Exception as e:
            message(module,str(e))
            print(str(e),file=sys.stderr)
            sys.exit(1)
        finally:
            notify_refresh(module)
            if module=='wifi' and args and args[0]=='connect':
                # Only stop the transient agent we own, never an existing desktop applet.
                subprocess.run(['systemctl','--user','stop','cc-wifi-agent.service'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
