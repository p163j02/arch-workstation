#!/bin/bash

mover_a_notebook() {
    sleep 0.5
    for i in 1 2 3 4 5 6; do
        hyprctl dispatch moveworkspacetomonitor "$i eDP-1"
    done
    hyprctl dispatch workspace 1
}

mover_a_hdmi() {
    sleep 1
    for i in 1 2 3 4 5 6; do
        hyprctl dispatch moveworkspacetomonitor "$i HDMI-A-1"
    done
    hyprctl dispatch workspace 1
}

# Al iniciar verificar estado
if ! hyprctl monitors | grep -q "HDMI-A-1"; then
    mover_a_notebook
else
    mover_a_hdmi
fi

handle() {
    case $1 in
        monitoradded*)   mover_a_hdmi ;;
        monitorremoved*) mover_a_notebook ;;
    esac
}

socat - "UNIX-CONNECT:$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket2.sock" | while read -r line; do
    handle "$line"
done
