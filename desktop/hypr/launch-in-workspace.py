#!/usr/bin/env python3
"""Launch a new app window on the workspace active when this script runs.

Pass the executable and its arguments separately. Hyprland's exec rule is
per-launch, not a permanent window rule, and does not move existing windows.
"""
import os
import shlex
import subprocess
import sys


def lua_string(value):
    """Quote UTF-8 bytes without allowing shell arguments to become Lua code."""
    return '"' + ''.join(chr(b) if 32 <= b < 127 and b not in (34, 92)
                         else f'\\{b:03d}' for b in value.encode('utf-8')) + '"'


def main():
    if len(sys.argv) < 2:
        print("Usage: launch-in-workspace.py COMMAND [ARGUMENT ...]", file=sys.stderr)
        return 2

    # Preserve desktop entries' working directories and quote each argument.
    command = f"cd {shlex.quote(os.getcwd())} && exec {shlex.join(sys.argv[1:])}"
    # Resolve and launch in one compositor request. Legacy `dispatch exec` is
    # no longer supported by Hyprland 0.57's Lua configuration provider.
    expression = (
        'local w = hl.get_active_workspace(); '
        'assert(w, "No active workspace"); '
        'local target = w.id >= 0 and tostring(w.id) or w.name; '
        f'hl.exec_cmd({lua_string(command)}, {{ workspace = target .. " silent" }})'
    )
    result = subprocess.run(
        ["hyprctl", "eval", expression],
        text=True, capture_output=True,
    )
    if result.returncode != 0 or result.stdout.strip() != "ok":
        print(result.stderr or result.stdout or "Hyprland launch failed", file=sys.stderr)
        return result.returncode or 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"Cannot launch application: {error}", file=sys.stderr)
        sys.exit(1)
