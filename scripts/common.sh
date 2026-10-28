#!/usr/bin/env bash

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"
CONFIG_NAMES=(btop eww fastfetch fish gtk-3.0 gtk-4.0 hypr kitty lazygit nvim
    nwg-dock-hyprland nwg-look omf opencode rofi swaync waybar waypaper wlogout
    wofi xsettingsd zed)
RSYNC_EXCLUDES=(--no-owner --no-group --exclude=node_modules/ --exclude=__pycache__/ --exclude=graphify-out/
    --exclude=target/ --exclude=.git/ --exclude='*.pyc' --exclude='*.bak*'
    --exclude='*.save*' --exclude=fish_variables --exclude=service.json
    --exclude=auth.json --exclude='.env*' --exclude='*.local.*' --exclude=fast-client
    --exclude='*.pem' --exclude='*.key' --exclude='*.p12' --exclude='*.pfx'
    --exclude='*.keystore' --exclude=id_rsa --exclude=id_ed25519 --exclude='.credentials*')

require_profile() {
    case "${1:-}" in
        laptop|desktop) PROFILE="$1" ;;
        *) echo "Uso: $0 laptop|desktop [--check|--config-only]" >&2; exit 2 ;;
    esac
}

read_packages() {
    sed -e 's/#.*//' -e '/^[[:space:]]*$/d' "$@"
}
