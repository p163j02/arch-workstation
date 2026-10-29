#!/usr/bin/env bash
set -Eeuo pipefail
status="$(playerctl -p spotify status 2>/dev/null)" || {
    printf '%s\n' '{"text":"","tooltip":"Spotify cerrado"}'; exit 0;
}
artist="$(playerctl -p spotify metadata artist 2>/dev/null || true)"
title="$(playerctl -p spotify metadata title 2>/dev/null || true)"
icon="󰝛"
[[ "$status" != Playing ]] || icon="󰝚"
jq -n --arg text "$icon" --arg tooltip "$artist - $title" --arg class "${status,,}" \
    '{text: $text, tooltip: $tooltip, class: $class}'
