#!/usr/bin/env bash
# Waybar 0.15.0 predates Hyprland's Lua IPC support; preserve the native module.
set -Eeuo pipefail
[[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]] || { echo 'Ejecuta desde una terminal de Hyprland.' >&2; exit 1; }
supports_lua() {
    python3 - "$(command -v waybar)" <<'PY'
from pathlib import Path
import sys
sys.exit(0 if b'hl.dsp' in Path(sys.argv[1]).read_bytes() else 1)
PY
}
if ! supports_lua; then
    command -v yay >/dev/null || { echo 'Falta yay para instalar waybar-git.' >&2; exit 1; }
    echo 'Se actualizará Waybar con el soporte Lua de upstream. Conserva tu configuración.'
    yay -S waybar-git
fi
supports_lua || { echo 'El ejecutable instalado todavía no incluye soporte Lua.' >&2; exit 1; }
hyprctl reload
systemctl --user restart waybar-control-dispatch.service
# Reloading with SIGUSR2 retains the old executable: replace the running process.
pkill -f '[/]hypr/monitor-handler-dynamic.sh' || true
pkill -TERM -x waybar || true
for _ in {1..50}; do
    pgrep -x waybar >/dev/null || break
    sleep 0.1
done
if pgrep -x waybar >/dev/null; then
    echo 'Waybar aún no terminó; no se iniciará una segunda instancia.' >&2
    exit 1
fi
log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/hyprland-recovery"
mkdir -p "$log_dir"
nohup "$HOME/.config/hypr/session-start.sh" > "$log_dir/waybar-restart.log" 2>&1 < /dev/null &
echo "Waybar actualizado. Registro del arranque: $log_dir/waybar-restart.log"
