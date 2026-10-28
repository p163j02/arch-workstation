#!/usr/bin/env bash
set -Eeuo pipefail
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
    [[ "$name" == hypr || "$name" == waybar ]] && continue
    sync_tree "$CONFIG_DIR/$name" "$SNAPSHOT/shared/$name"
done
for source in "$REPO_DIR/shared/hypr/"*; do
    name="${source##*/}"
    [[ ! -f "$CONFIG_DIR/hypr/$name" ]] || cp -a "$CONFIG_DIR/hypr/$name" "$SNAPSHOT/shared/hypr/$name"
done
hypr_excludes=(--exclude=local.conf)
for source in "$REPO_DIR/shared/hypr/"*; do hypr_excludes+=("--exclude=/${source##*/}"); done
sync_tree "$CONFIG_DIR/hypr" "$SNAPSHOT/$PROFILE/hypr" "${hypr_excludes[@]}"
sync_tree "$CONFIG_DIR/waybar" "$SNAPSHOT/$PROFILE/waybar" --exclude=/scripts/
sync_tree "$CONFIG_DIR/waybar/scripts" "$SNAPSHOT/shared/waybar/scripts"
[[ "$PROFILE" != laptop ]] || sync_tree "$CONFIG_DIR/nwg-displays" "$SNAPSHOT/laptop/nwg-displays"
[[ ! -f "$HOME/.nanorc" ]] || cp "$HOME/.nanorc" "$SNAPSHOT/shared/nano/user.nanorc"
for unit in "$REPO_DIR/shared/systemd/user/"*.service; do
    source="$CONFIG_DIR/systemd/user/${unit##*/}"
    [[ ! -f "$source" ]] || cp "$source" "$SNAPSHOT/shared/systemd/user/"
done
python3 "$REPO_DIR/scripts/portable.py" normalize "$SNAPSHOT/shared" "$HOME"
python3 "$REPO_DIR/scripts/portable.py" normalize "$SNAPSHOT/$PROFILE" "$HOME"
python3 "$REPO_DIR/scripts/check.py" --root "$SNAPSHOT"

rsync -a --delete "${RSYNC_EXCLUDES[@]}" "$SNAPSHOT/shared/" "$REPO_DIR/shared/"
rsync -a --delete "${RSYNC_EXCLUDES[@]}" "$SNAPSHOT/$PROFILE/" "$REPO_DIR/$PROFILE/"
echo "Actualizado el perfil $PROFILE. El otro perfil se conserva."
if git -C "$REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "$REPO_DIR" status --short
fi
