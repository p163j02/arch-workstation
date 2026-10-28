#!/bin/bash

BATTERY_DIR=""
for supply in /sys/class/power_supply/*; do
    if [[ -f "$supply/type" && "$(cat "$supply/type")" == Battery ]]; then
        BATTERY_DIR="$supply"
        break
    fi
done
[[ -n "$BATTERY_DIR" ]] || exit 0

LAST_NOTIFIED=100

while true; do
    BATTERY=$(cat "$BATTERY_DIR/capacity")
    STATUS=$(cat "$BATTERY_DIR/status")

    if [ "$STATUS" = "Charging" ]; then
        # Al conectar cargador, resetear para que avise de nuevo al desconectar
        LAST_NOTIFIED=100

    elif [ "$STATUS" = "Discharging" ]; then

        if [ "$BATTERY" -le 5 ] && [ "$LAST_NOTIFIED" -gt 5 ]; then
            notify-send -u critical "󰂃 Batería crítica" "${BATTERY}% — Suspendiendo en 60 segundos"
            LAST_NOTIFIED=5
            sleep 60
            # Verificar que sigue descargando antes de suspender
            if [[ "$(cat "$BATTERY_DIR/status")" == Discharging && "$(cat "$BATTERY_DIR/capacity")" -le 5 ]]; then
                systemctl suspend
            fi

        elif [ "$BATTERY" -le 10 ] && [ "$LAST_NOTIFIED" -gt 10 ]; then
            notify-send -u critical "󰁺 Batería muy baja" "${BATTERY}% — Conecta el cargador"
            LAST_NOTIFIED=10

        elif [ "$BATTERY" -le 20 ] && [ "$LAST_NOTIFIED" -gt 20 ]; then
            notify-send -u normal "󰁼 Batería baja" "${BATTERY}% — Conecta el cargador"
            LAST_NOTIFIED=20
        fi
    fi

    sleep 60
done
