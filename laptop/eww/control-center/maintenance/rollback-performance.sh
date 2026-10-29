#!/usr/bin/env bash
# Restore only files changed by persistent-dispatch optimization.
set -euo pipefail
root="$HOME/.config/eww/control-center"
backup="$HOME/.local/state/eww-performance/persistent-20261022-150610/control-center"
test -f "$backup/scripts/backend.py"
CC_DIRECT=1 "$root/scripts/popup-manager.py" close
systemctl --user disable --now waybar-control-dispatch.service
archive="$HOME/.local/state/eww-performance/before-persistent-rollback-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$archive"
for name in backend.py popup-manager.py; do
    cp -a "$root/scripts/$name" "$archive/$name"
    cp -a "$backup/scripts/$name" "$root/scripts/$name"
done
if test -f "$HOME/.config/systemd/user/waybar-control-dispatch.service"; then
    mv "$HOME/.config/systemd/user/waybar-control-dispatch.service" "$archive/"
fi
systemctl --user daemon-reload
printf 'Persistent dispatch disabled; scripts restored. Archived at %s\n' "$archive"
# New source, binary and tests are kept for inspection; restored scripts do not use them.
