#!/bin/bash

NOTEBOOK="eDP-1"

get_externos() {
  # 'monitors all' incluye también los que están en mirror o disabled
  hyprctl monitors all -j | python3 -c "
import json, sys
monitors = json.load(sys.stdin)
for m in monitors:
    if m['name'] != '$NOTEBOOK':
        print(m['name'])
"
}

modo_extender() {
  local EXT=$1
  hyprctl keyword monitor "$EXT,preferred,0x0,1"
  hyprctl keyword monitor "$NOTEBOOK,1920x1080@60,1920x0,1"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6; do
    batch+="dispatch moveworkspacetomonitor $ws $EXT ; "
  done
  for ws in 7 8 9 10 11 12; do
    batch+="dispatch moveworkspacetomonitor $ws $NOTEBOOK ; "
  done
  batch+="dispatch focusmonitor $EXT ; dispatch workspace 1 ; dispatch movecursor 960 540"
  hyprctl --batch "$batch"
  eww open sysmon-widget-ext 2>/dev/null
}

modo_espejo() {
  local EXT=$1
  hyprctl keyword monitor "$EXT,preferred,0x0,1,mirror,$NOTEBOOK"
  hyprctl keyword monitor "$NOTEBOOK,1920x1080@60,0x0,1"
  sleep 0.3
  hyprctl --batch "
        dispatch focusmonitor $NOTEBOOK ;
        dispatch workspace 1 ;
        dispatch movecursor 960 540
    "
  eww close sysmon-widget-ext 2>/dev/null
}

modo_solo_externo() {
  local EXT=$1
  # Habilitar EXT primero para que los workspaces tengan destino válido
  hyprctl keyword monitor "$EXT,preferred,0x0,1"
  sleep 0.5
  hyprctl keyword monitor "$NOTEBOOK,disabled"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6 7 8 9 10 11 12; do
    batch+="dispatch moveworkspacetomonitor $ws $EXT ; "
  done
  batch+="dispatch focusmonitor $EXT ; dispatch workspace 1 ; dispatch movecursor 960 540"
  hyprctl --batch "$batch"
  eww open sysmon-widget-ext 2>/dev/null
}

modo_solo_notebook() {
  local EXT=$1
  # Habilitar notebook ANTES de deshabilitar el externo para evitar que
  # los workspaces queden en limbo durante la transición
  hyprctl keyword monitor "$NOTEBOOK,1920x1080@60,0x0,1"
  sleep 0.5
  hyprctl keyword monitor "$EXT,disabled"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6 7 8 9 10 11 12; do
    batch+="dispatch moveworkspacetomonitor $ws $NOTEBOOK ; "
  done
  batch+="dispatch focusmonitor $NOTEBOOK ; dispatch workspace 1 ; dispatch movecursor 960 540"
  hyprctl --batch "$batch"
  eww close sysmon-widget-ext 2>/dev/null
}

reload_bars() {
  # Una sola instancia: la config de waybar ya declara todos los outputs
  pkill -SIGTERM waybar 2>/dev/null
  while pgrep -x waybar >/dev/null; do sleep 0.1; done
  sleep 0.5
  waybar -c ~/.config/waybar/config.jsonc -s ~/.config/waybar/style.css &
  disown
  wallpaper-studio --restore || waypaper --restore
}

# ── main ──────────────────────────────────────────────────────────────────────

externos=$(get_externos)

if [ -z "$externos" ]; then
  rofi -e "No hay monitores externos conectados." \
    -theme-str 'window {width: 350px;}'
  exit 0
fi

# Si hay más de uno, primero elegir cuál
if [ "$(echo "$externos" | wc -l)" -gt 1 ]; then
  EXT=$(echo "$externos" | rofi -dmenu \
    -p "¿Qué monitor?" \
    -theme-str 'window {width: 350px;} listview {lines: 4;}')
  [ -z "$EXT" ] && exit 0
else
  EXT="$externos"
fi

opcion=$(printf "󰿏  Extender área de trabajo\n󰹑  Espejo (mirror)\n󰍺  Solo externo (%s)\n󰌘  Solo notebook" "$EXT" |
  rofi -dmenu \
    -p "Modo: $EXT" \
    -theme-str 'window {width: 400px;} listview {lines: 4;}' \
    -i)

[ -z "$opcion" ] && exit 0

# Lock ANTES de ejecutar el modo, para que monitor-handler-dynamic ya lo vea
# cuando modo_extender dispara el evento monitoradded (evita waybar duplicado)
touch /tmp/monitor-switch-running
log_lock() { echo "[$(date +%H:%M:%S)] monitor-switch: lock activo" >>/tmp/monitor-handler.log; }
log_lock

case "$opcion" in
*"Extender"*)
  modo_extender "$EXT"
  reload_bars "$EXT eDP-1"
  ;;
*"Espejo"*)
  modo_espejo "$EXT"
  reload_bars "eDP-1"
  ;;
*"Solo externo"*)
  modo_solo_externo "$EXT"
  reload_bars "$EXT"
  ;;
*"Solo notebook"*)
  modo_solo_notebook "$EXT"
  reload_bars "eDP-1"
  ;;
esac

# Mantener el lock hasta que todo se estabilice (waybar tarda ~0.5–1s en arrancar)
sleep 2
rm -f /tmp/monitor-switch-running
