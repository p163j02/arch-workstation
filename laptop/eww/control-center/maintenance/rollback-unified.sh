#!/usr/bin/env bash
set -euo pipefail
state="$HOME/.local/state/waybar-control-center"
backup="$(<"$state/unified-backup")"
test -f "$backup/eww/control-center/eww.yuck"
archive="$state/before-unified-rollback-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$archive/waybar" "$archive/systemd-user"
cp -a "$HOME/.config/waybar/config.jsonc" "$archive/waybar/"
cp -a "$HOME/.config/systemd/user/waybar-control-center.service" "$archive/systemd-user/"
"$HOME/.config/eww/control-center/scripts/popup-manager.py" close || true
systemctl --user stop waybar-control-center
mv "$HOME/.config/eww/control-center" "$archive/control-center"
cp -a "$backup/eww/control-center" "$HOME/.config/eww/control-center"
cp -a "$backup/waybar/config.jsonc" "$HOME/.config/waybar/config.jsonc"
cp -a "$backup/systemd-user/waybar-control-center.service" "$HOME/.config/systemd/user/waybar-control-center.service"
systemctl --user daemon-reload
systemctl --user start waybar-control-center
pkill -SIGUSR2 -x waybar || true
printf 'Restaurado desde %s\nEstado anterior archivado en %s\n' "$backup" "$archive"
