#!/usr/bin/env bash
set -Eeuo pipefail
folder="$(xdg-user-dir PICTURES)/capturas"
mkdir -p "$folder"
file="$folder/$(date +%Y%m%d_%H%M%S_%N).png"
case "${1:-}" in
    area) selection="$(slurp)" || exit 0; grim -g "$selection" "$file" ;;
    full) grim "$file" ;;
    *) echo "Uso: $0 area|full" >&2; exit 2 ;;
esac
