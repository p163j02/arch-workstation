#!/usr/bin/env bash
set -Eeuo pipefail
exec "$HOME/.config/eww/control-center/scripts/backend.py" action nightlight warm
