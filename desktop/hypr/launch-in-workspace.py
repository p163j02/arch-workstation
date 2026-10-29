#!/usr/bin/env python3
"""Launch a new app window on the workspace active when this script runs.

Pass the executable and its arguments separately. Hyprland's exec rule is
per-launch, not a permanent window rule, and does not move existing windows.
"""
import json
import os
import shlex
import subprocess
import sys


def main():
    if len(sys.argv) < 2:
        print("Usage: launch-in-workspace.py COMMAND [ARGUMENT ...]", file=sys.stderr)
        return 2

    workspace = json.loads(subprocess.check_output(
        ["hyprctl", "-j", "activeworkspace"], text=True
    ))
    target = str(workspace["id"]) if workspace["id"] >= 0 else workspace["name"]
    if any(character in target for character in ",;[]\n\r"):
        raise ValueError("Unsupported workspace name")

    # Preserve desktop entries' working directories and quote each argument.
    command = f"cd {shlex.quote(os.getcwd())} && exec {shlex.join(sys.argv[1:])}"
    result = subprocess.run(
        ["hyprctl", "dispatch", "exec", f"[workspace {target} silent] {command}"],
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
