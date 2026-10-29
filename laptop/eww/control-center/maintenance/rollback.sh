#!/usr/bin/env bash
set -euo pipefail
state="$HOME/.local/state/waybar-control-center"
backup="$(<"$state/latest-backup")"
for file in waybar/config.jsonc waybar/style.css hypr/hyprland.conf; do
    test -f "$backup/$file" || { printf 'Falta respaldo: %s\n' "$backup/$file" >&2; exit 1; }
done
new="$state/before-rollback-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$new/waybar" "$new/hypr"
for file in waybar/config.jsonc waybar/style.css hypr/hyprland.conf; do
    cp -a "$HOME/.config/$file" "$new/$file"
done
"$HOME/.config/eww/control-center/scripts/popup-manager.py" close || true
systemctl --user disable --now waybar-control-center.service airpods-control-center.service
for file in waybar/config.jsonc waybar/style.css hypr/hyprland.conf; do
    cp -a "$backup/$file" "$HOME/.config/$file"
done
hyprctl reload
pkill -SIGUSR2 -x waybar || true
printf 'Restaurado desde %s\nRespaldo anterior a reversión: %s\n' "$backup" "$new"
