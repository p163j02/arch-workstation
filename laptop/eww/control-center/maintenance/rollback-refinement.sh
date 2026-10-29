#!/usr/bin/env bash
# Revert only this refinement, preserving the initial six-panel installation.
set -euo pipefail
state="$HOME/.local/state/waybar-control-center"
backup="$(<"$state/refinement-backup")"
test -f "$backup/waybar/config.jsonc"
test -f "$backup/eww/control-center/eww.yuck"
archive="$state/before-refinement-rollback-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$archive/waybar"
cp -a "$HOME/.config/waybar/config.jsonc" "$archive/waybar/"
"$HOME/.config/eww/control-center/scripts/popup-manager.py" close || true
systemctl --user stop waybar-control-center.service
mv "$HOME/.config/eww/control-center" "$archive/control-center"
cp -a "$backup/eww/control-center" "$HOME/.config/eww/control-center"
cp -a "$backup/waybar/config.jsonc" "$HOME/.config/waybar/config.jsonc"
systemctl --user start waybar-control-center.service
pkill -SIGUSR2 -x waybar || true
printf 'Restaurado desde %s\nEstado previo conservado en %s\n' "$backup" "$archive"
