#!/bin/sh
# Wallpaper Studio shares the Rust/GTK4 build with Waybar Editor.
set -eu
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec sh "$app_dir/../waybar-editor/install.sh"
