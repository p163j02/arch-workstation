#!/usr/bin/env bash
set -Eeuo pipefail
dbus-update-activation-environment --systemd WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE
systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE
systemctl --user daemon-reload
systemctl --user start hyprpolkitagent.service waybar-workspaces.service waybar-control-center.service \
    waybar-control-dispatch.service waybar-wifi-portal.service || echo "Revisa los servicios de usuario en journalctl --user." >&2
if [[ -x "$HOME/.local/bin/airpods-tui" ]]; then
    systemctl --user start airpods-control-center.service
fi
if ! awww query >/dev/null 2>&1; then
    awww-daemon &
    for _ in {1..50}; do
        awww query >/dev/null 2>&1 && break
        sleep 0.1
    done
fi
wallpaper-studio --restore || waypaper --restore || echo "No se pudo restaurar el fondo; revisa Wallpaper Studio." >&2
eww daemon || echo "Revisa el daemon Eww." >&2
pc-settings --open-sysmon sysmon-widget || eww open sysmon-widget || echo "No se pudo abrir el monitor de recursos." >&2
if [[ $(hyprctl monitors -j | jq length) -gt 1 ]]; then pc-settings --open-sysmon sysmon-widget-ext || eww open sysmon-widget-ext || true; fi
if [[ -x "$HOME/.config/hypr/monitor-handler-dynamic.sh" ]]; then
    "$HOME/.config/hypr/monitor-handler-dynamic.sh" &
else
    waybar &
fi
