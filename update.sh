#!/usr/bin/env bash
set -Eeuo pipefail
if [[ ${1:-} == laptop ]]; then
    shift
    exec bash "$(dirname -- "${BASH_SOURCE[0]}")/laptop/update.sh" "$@"
fi
source "$(dirname -- "${BASH_SOURCE[0]}")/scripts/common.sh"
require_profile "${1:-}"
[[ $# -eq 1 ]] || { echo "Uso: $0 laptop|desktop" >&2; exit 2; }
if [[ -f "$CONFIG_DIR/dotfiles/profile" ]]; then
    read -r installed < "$CONFIG_DIR/dotfiles/profile"
    [[ "$installed" == "$PROFILE" ]] || {
        echo "Este equipo usa $installed; no se sobrescribirá el perfil $PROFILE." >&2; exit 1;
    }
else
    echo "Primero instala este formato con install.sh $PROFILE --config-only; falta el marcador del perfil." >&2
    exit 1
fi
command -v rsync >/dev/null || { echo "Falta rsync." >&2; exit 1; }
SNAPSHOT="$(mktemp -d -t dotfiles-update.XXXXXXXX)"
trap 'rm -rf -- "$SNAPSHOT"' EXIT
rsync -a "${RSYNC_EXCLUDES[@]}" "$REPO_DIR/" "$SNAPSHOT/"

sync_tree() {
    local source="$1" destination="$2"
    shift 2
    if [[ ! -d "$source" ]]; then
        echo "Aviso: no existe $source; se conserva el respaldo anterior." >&2
        return
    fi
    mkdir -p "$destination"
    rsync -a --delete "${RSYNC_EXCLUDES[@]}" "$@" "$source/" "$destination/"
}
for name in "${CONFIG_NAMES[@]}"; do
    # Older installations have only .conf; retain the new Lua entry points until applied.
    native_excludes=()
    if [[ "$name" == hypr && ! -f "$CONFIG_DIR/hypr/hyprland.lua" ]]; then
        native_excludes=(--exclude=hyprland.lua --exclude=local-settings.lua)
    fi
    sync_tree "$CONFIG_DIR/$name" "$SNAPSHOT/$PROFILE/$name" "${native_excludes[@]}" \
        --exclude=local.conf --exclude=local.lua --exclude='*.so' \
        --exclude=workspaces.conf --exclude=workspaces-state.css
done
[[ ! -d "$REPO_DIR/$PROFILE/nwg-displays" ]] || sync_tree "$CONFIG_DIR/nwg-displays" "$SNAPSHOT/$PROFILE/nwg-displays"
sync_tree "$HOME/wallpapers" "$SNAPSHOT/$PROFILE/wallpapers"
[[ ! -f "$HOME/.nanorc" ]] || cp "$HOME/.nanorc" "$SNAPSHOT/$PROFILE/nano/user.nanorc"
for unit in "$REPO_DIR/$PROFILE/systemd/user/"*.service; do
    source="$CONFIG_DIR/systemd/user/${unit##*/}"
    [[ ! -f "$source" ]] || cp "$source" "$SNAPSHOT/$PROFILE/systemd/user/"
done
python3 "$REPO_DIR/scripts/portable.py" normalize "$SNAPSHOT/$PROFILE" "$HOME"
python3 "$REPO_DIR/scripts/check.py" --root "$SNAPSHOT"

rsync -a --delete "${RSYNC_EXCLUDES[@]}" "$SNAPSHOT/$PROFILE/" "$REPO_DIR/$PROFILE/"
echo "Actualizado el perfil $PROFILE. El otro perfil se conserva."
if git -C "$REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "$REPO_DIR" status --short
fi
