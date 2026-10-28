#!/usr/bin/env python3
"""Choose an installed settings app; launch independently of Eww timeouts."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CHOICES = {
    'wifi': [
        ('NetworkManager', 'Editar conexiones y perfiles', ['nm-connection-editor']),
        ('nmtui', 'Gestor de Wi-Fi con interfaz de terminal', ['kitty', '--class', 'nmtui', '--title', 'nmtui', '-e', 'nmtui']),
    ],
    'volume': [
        ('Pavucontrol', 'Control gráfico de volumen', ['pavucontrol']),
        ('Pulsemixer', 'Mezclador de audio en terminal', ['kitty', '--title', 'Pulsemixer', '-e', 'pulsemixer']),
    ],
}


def available_choices(kind):
    return [(name, description, argv) for name, description, argv in CHOICES[kind]
            if shutil.which(argv[0]) and (argv[0] != 'kitty' or shutil.which(argv[-1]))]


def launch_app(argv):
    # No wait(), inherited Eww pipes, or timeout: the app owns its lifecycle.
    return subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL, close_fds=True, start_new_session=True)


def main(kind):
    if kind not in CHOICES:
        raise ValueError('Configuración no válida')
    subprocess.run([sys.executable, str(ROOT / 'scripts/popup-manager.py'), 'close'],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=8)
    import gi
    gi.require_version('Gtk', '3.0')
    from gi.repository import Gtk
    window = Gtk.Window(title='Elegir gestor de ' + ('Wi-Fi' if kind == 'wifi' else 'audio'))
    window.set_default_size(410, -1)
    window.set_position(Gtk.WindowPosition.CENTER)
    window.set_border_width(20)
    window.connect('destroy', Gtk.main_quit)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
    window.add(box)
    heading = Gtk.Label(label='¿Qué gestor quieres abrir?')
    heading.set_xalign(0)
    box.pack_start(heading, False, False, 0)
    for name, description, argv in available_choices(kind):
        button = Gtk.Button(label=name + '\n' + description)
        button.set_tooltip_text(description)
        def choose(_button, command=argv):
            launch_app(command)
            window.destroy()
        button.connect('clicked', choose)
        box.pack_start(button, False, False, 0)
    cancel = Gtk.Button(label='Cancelar')
    cancel.connect('clicked', lambda *_: window.destroy())
    box.pack_start(cancel, False, False, 0)
    window.show_all()
    Gtk.main()


if __name__ == '__main__':
    kind = sys.argv[1] if len(sys.argv) == 2 else ''
    if kind not in CHOICES:
        sys.exit('Configuración no válida')
    # Eww terminates its foreground command after five seconds. Detach BEFORE
    # constructing GTK; both the chooser and the selected app survive that limit.
    if os.fork():
        sys.exit(0)
    os.setsid()
    with open(os.devnull, 'r+b', buffering=0) as null:
        for descriptor in (0, 1, 2):
            os.dup2(null.fileno(), descriptor)
    main(kind)
