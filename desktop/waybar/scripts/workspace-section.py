#!/usr/bin/env python3
"""Visibility of the native extra workspaces; never moves or closes windows."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import time
from dataclasses import dataclass

def runtime_dir():
    signature = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", "no-session")
    token = hashlib.sha256(signature.encode()).hexdigest()[:12]
    base = Path(os.environ.get("XDG_RUNTIME_DIR", f"/tmp/waybar-{os.getuid()}"))
    path = base / f"waybar-workspaces-{token}"
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    return path


def atomic_write(path, text):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(text)
    temporary.replace(path)


@dataclass
class Section:
    mode: str = "auto"
    active: int = 1
    occupied: bool = False

    @property
    def expanded(self):
        return self.mode == "pinned" or (
            self.mode == "auto" and (self.active >= 7 or self.occupied)
        )

    def refresh(self, active, clients):
        # A deliberate new visit reopens a section that was manually hidden.
        entering_extra = active >= 7 and active != self.active
        self.active = active
        self.occupied = any(c.get("workspace", {}).get("id", 0) >= 7 for c in clients)
        if self.mode == "hidden" and (
            entering_extra or (active < 7 and not self.occupied)
        ):
            self.mode = "auto"

    def toggle(self):
        self.mode = "hidden" if self.expanded else "pinned"

    def status(self):
        if self.expanded:
            detail = "Fijados" if self.mode == "pinned" else "Expansión automática"
            text = ""
            tooltip = f"{detail} · Ocultar α en adelante sin cerrar ventanas"
        else:
            text = ""
            tooltip = "Mostrar y fijar α en adelante hasta volver a pulsar"
        return {"text": text, "tooltip": tooltip, "class": self.mode}


def hypr_json(command):
    result = subprocess.run(
        ["hyprctl", "-j", command], capture_output=True, text=True, timeout=3, check=True
    )
    return json.loads(result.stdout)


def snapshot(section):
    active = hypr_json("activeworkspace")["id"]
    clients = hypr_json("clients")
    section.refresh(active, clients)


def publish(section, directory):
    # Only state changes wake the native GTK adapter and the button stream.
    path = directory / "state.json"
    contents = json.dumps({
        "mode": section.mode, "active": section.active,
        "occupied": section.occupied, "expanded": section.expanded,
        "status": section.status(),
    }, ensure_ascii=False) + "\n"
    if not path.exists() or path.read_text() != contents:
        atomic_write(path, contents)


def watch_status():
    # A persistent stream replaces a Python process and a stale icon every second.
    import gi
    from gi.repository import Gio, GLib

    directory = runtime_dir()
    last = None

    def emit(*_):
        nonlocal last
        try:
            status = json.loads((directory / "state.json").read_text())["status"]
        except (OSError, ValueError, KeyError):
            status = Section().status()
        text = json.dumps(status, ensure_ascii=False)
        if text != last:
            print(text, flush=True)
            last = text

    monitor = Gio.File.new_for_path(str(directory)).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, None)
    monitor.set_rate_limit(0)
    monitor.connect("changed", emit)
    emit()
    GLib.MainLoop().run()


def connect_events():
    signature = os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")
    if not signature:
        raise OSError("HYPRLAND_INSTANCE_SIGNATURE no está definido")
    base = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}"))
    events = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    events.settimeout(3)
    try:
        events.connect(str(base / "hypr" / signature / ".socket2.sock"))
    except OSError:
        events.close()
        raise
    events.setblocking(False)
    return events


EVENTS = {
    "workspace", "workspacev2", "focusedmon", "focusedmonv2", "openwindow",
    "closewindow", "movewindow", "movewindowv2", "createworkspace",
    "createworkspacev2", "destroyworkspace", "destroyworkspacev2",
    "moveworkspace", "moveworkspacev2", "monitoradded", "monitorremoved",
}


def daemon():
    directory = runtime_dir()
    with (directory / "daemon.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return
        state = Section()
        try:
            previous = json.loads((directory / "state.json").read_text())
            if previous["mode"] in {"auto", "pinned", "hidden"}:
                state = Section(previous["mode"], previous["active"], previous["occupied"])
        except (OSError, KeyError, ValueError):
            pass
        control_path = directory / "control.sock"
        control_path.unlink(missing_ok=True)
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as control:
            control.bind(str(control_path))
            control.listen(8)
            events = None
            buffer = b""
            next_refresh = 0
            last_error = None
            try:
                while True:
                    if events is None:
                        try:
                            # Subscribe before taking the snapshot to avoid missed events.
                            events = connect_events()
                            buffer = b""
                            next_refresh = 0
                        except OSError:
                            pass
                    now = time.monotonic()
                    if now >= next_refresh:
                        try:
                            snapshot(state)
                            publish(state, directory)
                            last_error = None
                        except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
                            message = str(error)
                            if message != last_error:
                                print(f"workspaces: {message}", file=sys.stderr, flush=True)
                                last_error = message
                        # Also recover from missed events or a restarted compositor.
                        next_refresh = now + (5 if events else 1)
                    ready, _, _ = select.select([control] + ([events] if events else []), [], [], 1)
                    if control in ready:
                        with control.accept()[0] as connection:
                            connection.settimeout(1)
                            try:
                                command = connection.recv(64).decode().strip()
                                if command == "toggle":
                                    # Reconcile pending compositor changes before toggling.
                                    try:
                                        snapshot(state)
                                    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
                                        pass
                                    state.toggle()
                                    publish(state, directory)
                                    connection.sendall(b"ok\n")
                            except (OSError, UnicodeError):
                                pass
                    if events is not None and events in ready:
                        try:
                            chunk = events.recv(65536)
                        except OSError:
                            chunk = b""
                        if not chunk:
                            events.close()
                            events = None
                        else:
                            buffer += chunk
                            lines = buffer.split(b"\n")
                            buffer = lines.pop()
                            if any(line.split(b">>", 1)[0].decode(errors="replace") in EVENTS for line in lines):
                                next_refresh = 0
            finally:
                if events is not None:
                    events.close()
                control_path.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["daemon", "status", "watch", "toggle"])
    args = parser.parse_args()
    if args.action == "daemon":
        daemon()
    elif args.action == "watch":
        watch_status()
    elif args.action == "status":
        try:
            status = json.loads((runtime_dir() / "state.json").read_text())["status"]
        except (OSError, ValueError, KeyError):
            status = Section().status()
        print(json.dumps(status, ensure_ascii=False))
    else:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as control:
            control.settimeout(3)
            try:
                control.connect(str(runtime_dir() / "control.sock"))
                control.sendall(b"toggle\n")
                if control.recv(64).strip() != b"ok":
                    raise OSError("el controlador no confirmó el cambio")
            except OSError as error:
                print(f"No se pudo alternar la sección: {error}. Inicia waybar-workspaces.service.", file=sys.stderr)
                return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
