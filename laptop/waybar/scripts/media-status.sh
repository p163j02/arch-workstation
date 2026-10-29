#!/bin/bash
STATUS=$(playerctl -p spotify status 2>/dev/null)

if [ -z "$STATUS" ]; then
    echo '{"text": "", "tooltip": "Spotify cerrado"}'
    exit 0
fi

ARTIST=$(playerctl -p spotify metadata artist 2>/dev/null)
TITLE=$(playerctl -p spotify metadata title 2>/dev/null)

if [ "$STATUS" = "Playing" ]; then
    ICON="󰝚"
else
    ICON="󰝛"
fi

# Escapar comillas por si el título las tiene
TOOLTIP=$(echo "$ARTIST - $TITLE" | sed 's/"/\\"/g')

echo "{\"text\": \"$ICON\", \"tooltip\": \"$TOOLTIP\", \"class\": \"$(echo $STATUS | tr '[:upper:]' '[:lower:]')\"}"
