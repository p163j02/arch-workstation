#!/bin/bash

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

mover_a_notebook() {
    esperar_monitor "HDMI-A-1" "desconectado" || return
    echo "[$(date +%H:%M:%S)] monitorremoved detectado" >> /tmp/monitor-handler.log

    "$HOME/.config/hypr/monitor-set.py" "eDP-1,1920x1080@60,0x0,1"
    sleep 0.3

    hyprctl --batch "
        dispatch hl.dsp.workspace.move({workspace='1', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='2', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='3', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='4', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='5', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='6', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='7', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='8', monitor='eDP-1'}) ;
        dispatch hl.dsp.focus({monitor='eDP-1'}) ;
        dispatch hl.dsp.focus({workspace='1'}) ;
        dispatch hl.dsp.cursor.move({x=960, y=540})
    "

    echo "[$(date +%H:%M:%S)] batch ejecutado, cursorpos: $(hyprctl cursorpos)" >> /tmp/monitor-handler.log

    sleep 0.3
    pkill waybar; sleep 0.2; waybar &disown
    waypaper --restore
    eww close sysmon-widget-ext 2>/dev/null
    echo "[$(date +%H:%M:%S)] finalizado" >> /tmp/monitor-handler.log
}

mover_a_hdmi() {
    esperar_monitor "HDMI-A-1" "conectado" || return
    echo "[$(date +%H:%M:%S)] monitoradded detectado" >> /tmp/monitor-handler.log

    "$HOME/.config/hypr/monitor-set.py" "HDMI-A-1,preferred,0x0,1"
    "$HOME/.config/hypr/monitor-set.py" "eDP-1,1920x1080@60,1920x0,1"
    sleep 0.3

    hyprctl --batch "
        dispatch hl.dsp.workspace.move({workspace='1', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='2', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='3', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='4', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='5', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='6', monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='7', monitor='eDP-1'}) ;
        dispatch hl.dsp.workspace.move({workspace='8', monitor='eDP-1'}) ;
        dispatch hl.dsp.focus({monitor='HDMI-A-1'}) ;
        dispatch hl.dsp.focus({workspace='1'}) ;
        dispatch hl.dsp.cursor.move({x=960, y=540})
    "

    echo "[$(date +%H:%M:%S)] batch ejecutado, cursorpos: $(hyprctl cursorpos)" >> /tmp/monitor-handler.log

    sleep 0.3
    pkill waybar; sleep 0.2; waybar &disown
    waypaper --restore
    eww open sysmon-widget-ext 2>/dev/null
    echo "[$(date +%H:%M:%S)] finalizado" >> /tmp/monitor-handler.log
}

# Al iniciar
if ! hyprctl monitors | grep -q "HDMI-A-1"; then
    mover_a_notebook
else
    mover_a_hdmi
fi

handle() {
    case $1 in
        monitoradded*)   mover_a_hdmi     ;;
        monitorremoved*) mover_a_notebook ;;
    esac
}

socat -u "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" - | while read -r line; do
    handle "$line"
done
