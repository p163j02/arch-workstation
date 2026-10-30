#!/bin/bash

NOTEBOOK="eDP-1"
LOG="/tmp/monitor-handler.log"

log() { echo "[$(date +%H:%M:%S)] $*" >>"$LOG"; }

# ── helpers ──────────────────────────────────────────────────────────────────

get_externos() {
  hyprctl monitors -j | python3 -c "
import json, sys
monitors = json.load(sys.stdin)
for m in monitors:
    if m['name'] != '$NOTEBOOK':
        print(m['name'])
"
}

esperar_monitor() {
  local monitor=$1
  local estado=$2
  local intentos=0
  while [ $intentos -lt 30 ]; do
    if [ "$estado" = "conectado" ] && hyprctl monitors | grep -q "$monitor"; then
      return 0
    elif [ "$estado" = "desconectado" ] && ! hyprctl monitors | grep -q "$monitor"; then
      return 0
    fi
    sleep 0.1
    intentos=$((intentos + 1))
  done
  return 1
}

reload_bars() {
  # Una sola instancia: la config de waybar ya declara todos los outputs
  pkill -SIGTERM waybar 2>/dev/null
  while pgrep -x waybar >/dev/null; do sleep 0.1; done
  sleep 0.3
  waybar -c ~/.config/waybar/config.jsonc -s ~/.config/waybar/style.css &
  disown
  wallpaper-studio --restore || waypaper --restore
}

# ── modos de pantalla ────────────────────────────────────────────────────────

modo_extender() {
  local EXT=$1
  log "modo: extender ($EXT)"

  "$HOME/.config/hypr/monitor-set.py" "$EXT,preferred,0x0,1"
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,1920x1080@60,1920x0,1"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6; do
    batch+="dispatch hl.dsp.workspace.move({workspace='$ws', monitor='$EXT'}) ; "
  done
  for ws in 7 8 9 10 11 12; do
    batch+="dispatch hl.dsp.workspace.move({workspace='$ws', monitor='$NOTEBOOK'}) ; "
  done
  batch+="dispatch hl.dsp.focus({monitor='$EXT'}) ; dispatch hl.dsp.focus({workspace='1'}) ; dispatch hl.dsp.cursor.move({x=960, y=540})"

  hyprctl --batch "$batch"
  eww open sysmon-widget-ext 2>/dev/null
}

modo_espejo() {
  local EXT=$1
  log "modo: espejo ($EXT)"

  "$HOME/.config/hypr/monitor-set.py" "$EXT,preferred,0x0,1,mirror,$NOTEBOOK"
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,1920x1080@60,0x0,1"
  sleep 0.3

  # En espejo el foco se queda en el notebook
  hyprctl --batch "
        dispatch hl.dsp.focus({monitor='$NOTEBOOK'}) ;
        dispatch hl.dsp.focus({workspace='1'}) ;
        dispatch hl.dsp.cursor.move({x=960, y=540})
    "
  eww close sysmon-widget-ext 2>/dev/null
}

modo_solo_externo() {
  local EXT=$1
  log "modo: solo externo ($EXT)"

  # Habilitar EXT antes de deshabilitar notebook para que los workspaces
  # tengan destino válido durante la transición
  "$HOME/.config/hypr/monitor-set.py" "$EXT,preferred,0x0,1"
  sleep 0.5
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,disabled"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6 7 8 9 10 11 12; do
    batch+="dispatch hl.dsp.workspace.move({workspace='$ws', monitor='$EXT'}) ; "
  done
  batch+="dispatch hl.dsp.focus({monitor='$EXT'}) ; dispatch hl.dsp.focus({workspace='1'}) ; dispatch hl.dsp.cursor.move({x=960, y=540})"

  hyprctl --batch "$batch"
  eww open sysmon-widget-ext 2>/dev/null
}

modo_solo_notebook() {
  local EXT=$1
  log "modo: solo notebook (deshabilitando $EXT)"

  # Habilitar notebook ANTES de deshabilitar el externo
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,1920x1080@60,0x0,1"
  sleep 0.5
  "$HOME/.config/hypr/monitor-set.py" "$EXT,disabled"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6 7 8 9 10 11 12; do
    batch+="dispatch hl.dsp.workspace.move({workspace='$ws', monitor='$NOTEBOOK'}) ; "
  done
  batch+="dispatch hl.dsp.focus({monitor='$NOTEBOOK'}) ; dispatch hl.dsp.focus({workspace='1'}) ; dispatch hl.dsp.cursor.move({x=960, y=540})"

  hyprctl --batch "$batch"
  eww close sysmon-widget-ext 2>/dev/null
}

elegir_modo() {
  local EXT=$1
  local opcion

  opcion=$(printf "󰿏  Extender área de trabajo\n  Espejo (mirror)\n󰍺  Solo externo (%s)\n  Solo notebook" "$EXT" |
    rofi -dmenu \
      -p "Monitor: $EXT" \
      -theme-str 'window {width: 400px;} listview {lines: 4;}' \
      -i)

  # Doble verificación: si monitor-switch.sh ya se encargó mientras el menú
  # estaba abierto (o antes de que llegáramos aquí), no relanzar waybar
  if [ -f /tmp/monitor-switch-running ]; then
    log "elegir_modo: lock activo al ejecutar modo, omitiendo waybar"
    return
  fi

  case "$opcion" in
  *"Extender"*)
    modo_extender "$EXT"
    reload_bars "$EXT $NOTEBOOK"
    ;;
  *"Espejo"*)
    modo_espejo "$EXT"
    reload_bars "$NOTEBOOK"
    ;;
  *"Solo externo"*)
    modo_solo_externo "$EXT"
    reload_bars "$EXT"
    ;;
  *"Solo notebook"*)
    modo_solo_notebook "$EXT"
    reload_bars "$NOTEBOOK"
    ;;
  *)
    log "menú cancelado — usando extender por defecto"
    modo_extender "$EXT"
    reload_bars "$EXT $NOTEBOOK"
    ;;
  esac
}

# ── lógica principal ──────────────────────────────────────────────────────────

monitor_conectado() {
  local EXT=$1
  esperar_monitor "$EXT" "conectado" || return
  log "monitoradded: $EXT"

  # Si monitor-switch.sh está gestionando el cambio, él ya muestra el menú
  # y gestiona waybar — no hacer nada aquí para evitar duplicados
  if [ -f /tmp/monitor-switch-running ]; then
    log "switch manual en curso — omitiendo menú y waybar para $EXT"
    return
  fi

  # elegir_modo llama a reload_bars internamente según el modo elegido
  elegir_modo "$EXT"
  log "finalizado (externo: $EXT)"
}

monitor_desconectado() {
  local EXT=$1
  esperar_monitor "$EXT" "desconectado" || return
  log "monitorremoved: $EXT"

  # Si aún hay otros externos conectados, no hacer nada (caso: tenías 2 y desconectaste 1)
  local restantes
  restantes=$(get_externos)
  if [ -n "$restantes" ]; then
    log "aún hay externos conectados: $restantes — sin cambios"
    return
  fi

  # Solo queda el notebook
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,1920x1080@60,0x0,1"
  sleep 0.3

  local batch=""
  for ws in 1 2 3 4 5 6 7 8 9 10 11 12; do
    batch+="dispatch hl.dsp.workspace.move({workspace='$ws', monitor='$NOTEBOOK'}) ; "
  done
  batch+="dispatch hl.dsp.focus({monitor='$NOTEBOOK'}) ; dispatch hl.dsp.focus({workspace='1'}) ; dispatch hl.dsp.cursor.move({x=960, y=540})"

  hyprctl --batch "$batch"
  log "batch ejecutado, cursorpos: $(hyprctl cursorpos)"

  if [ -f /tmp/monitor-switch-running ]; then
    log "switch manual en curso — omitiendo reload waybar"
  else
    reload_bars "$NOTEBOOK"
  fi
  eww close sysmon-widget-ext 2>/dev/null
  log "finalizado (solo notebook)"
}

# ── al iniciar ────────────────────────────────────────────────────────────────
# Al inicio no se muestra el menú — si hay externos se extiende directamente

externos_al_inicio=$(get_externos)
if [ -z "$externos_al_inicio" ]; then
  log "inicio: solo notebook"
  "$HOME/.config/hypr/monitor-set.py" "$NOTEBOOK,1920x1080@60,0x0,1"
  reload_bars "$NOTEBOOK"
else
  log "inicio: externos detectados → $externos_al_inicio (extendiendo sin menú)"
  for ext in $externos_al_inicio; do
    esperar_monitor "$ext" "conectado" || continue
    modo_extender "$ext"
    log "finalizado inicio (externo: $ext)"
  done
  # Lanzar waybar para todos los monitores activos tras configurar todos
  all_active=$(hyprctl monitors -j | python3 -c "
import json, sys
print(' '.join(m['name'] for m in json.load(sys.stdin)))
")
  reload_bars "$all_active"
fi

# ── listener ─────────────────────────────────────────────────────────────────

handle() {
  local line=$1
  case $line in
  monitoradded\>\>*)
    local monitor="${line#monitoradded>>}"
    [ "$monitor" != "$NOTEBOOK" ] && monitor_conectado "$monitor"
    ;;
  monitorremoved\>\>*)
    local monitor="${line#monitorremoved>>}"
    [ "$monitor" != "$NOTEBOOK" ] && monitor_desconectado "$monitor"
    ;;
  esac
}

socat -u "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" - |
  while read -r line; do
    handle "$line"
  done
