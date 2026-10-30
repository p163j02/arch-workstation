#!/bin/sh
# The settings shell embeds the existing Rust applications in the same process.
set -eu
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec sh "$app_dir/../waybar-editor/install.sh"
