#!/usr/bin/env python3
"""Request normal window closure by workspace group; never force-kill processes."""

import argparse
import json
import re
import subprocess
import sys


def select_clients(clients, group):
    def selected(client):
        workspace = client.get("workspace", {}).get("id", 0)
        return group == "all" or (1 <= workspace <= 6 if group == "main" else workspace >= 7)
    return [c for c in clients if selected(c) and re.fullmatch(r"0x[0-9a-fA-F]+", c.get("address", ""))]


def close_commands(clients):
    return [f"dispatch closewindow address:{client['address']}" for client in clients]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("group", choices=["main", "extra", "all"])
    parser.add_argument("--dry-run", action="store_true", help="Lista ventanas sin cerrarlas")
    args = parser.parse_args()
    try:
        result = subprocess.run(["hyprctl", "-j", "clients"], capture_output=True, text=True, check=True, timeout=3)
        # Snapshot by address: changing focus does not affect which windows close.
        clients = select_clients(json.loads(result.stdout), args.group)
        if args.dry_run:
            print(json.dumps(clients, ensure_ascii=False, indent=2))
        elif clients:
            result = subprocess.run(["hyprctl", "--batch", "; ".join(close_commands(clients))],
                                    capture_output=True, text=True, check=True, timeout=15)
            responses = result.stdout.splitlines()
            if any(line.strip() and line.strip() != "ok" for line in responses):
                print(result.stdout, file=sys.stderr)
                return 1
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"No se pudieron cerrar las ventanas: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
