#!/bin/bash

LAST_NOTIFIED=100

while true; do
    BATTERY=$(cat /sys/class/power_supply/BAT0/capacity)
    STATUS=$(cat /sys/class/power_supply/BAT0/status)

    if [ "$STATUS" = "Charging" ]; then
        # Al conectar cargador, resetear para que avise de nuevo al desconectar
        LAST_NOTIFIED=100

    elif [ "$STATUS" = "Discharging" ]; then

        if [ "$BATTERY" -le 5 ] && [ "$LAST_NOTIFIED" -gt 5 ]; then
            notify-send -u critical "󰂃 Batería crítica" "${BATTERY}% — Suspendiendo en 60 segundos"
            LAST_NOTIFIED=5
            sleep 60
            # Verificar que sigue descargando antes de suspender
            [ "$(cat /sys/class/power_supply/BAT0/status)" = "Discharging" ] && systemctl suspend

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
