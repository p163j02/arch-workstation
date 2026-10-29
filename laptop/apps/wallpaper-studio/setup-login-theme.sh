#!/usr/bin/env bash
set -Eeuo pipefail
[[ $EUID -ne 0 ]] || { echo 'Ejecuta este instalador como tu usuario, sin sudo.' >&2; exit 1; }
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
binary="$HOME/.local/bin/wallpaper-studio"
if [[ ! -x "$binary" ]] || ! "$binary" --help | grep -Fq -- '--sync-login'; then
    echo 'Actualiza primero Wallpaper Studio con bash apps/wallpaper-studio/install.sh.' >&2
    exit 1
fi
theme=/usr/share/sddm/themes/pixie
state="${XDG_STATE_HOME:-$HOME/.local/state}/wallpaper-studio"
config="${XDG_CONFIG_HOME:-$HOME/.config}/hypr"
public="/var/lib/wallpaper-studio/$(id -u)"
[[ -f "$theme/Main.qml" && -f "$theme/theme.conf" ]] || { echo 'Falta el tema Pixie de SDDM.' >&2; exit 1; }
[[ ! -L "$config/hyprlock.conf" ]] || { echo 'No se reemplaza un hyprlock.conf simbólico.' >&2; exit 1; }
if [[ -e "$state/login-theme" || -L "$state/login-theme" ]]; then
    [[ -L "$state/login-theme" && $(readlink "$state/login-theme") == "$public" ]] || {
        echo 'La ruta login-theme ya existe y no pertenece a esta integración.' >&2; exit 1;
    }
fi
mkdir -p "$state" "$config"
backup=$(mktemp -d "$state/login-backup.XXXXXXXX")
for file in Main.qml components/Clock.qml theme.conf.user; do
    if [[ -e "$theme/$file" ]]; then cp -pL "$theme/$file" "$backup/${file##*/}"; fi
done
[[ ! -f "$config/hyprlock.conf" ]] || cp -p "$config/hyprlock.conf" "$backup/hyprlock.conf"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
python3 "$app_dir/sddm_theme.py" "$theme" "$stage"
sudo install -d -m755 -o root -g root /var/lib/wallpaper-studio
sudo install -d -m755 -o "$(id -u)" -g "$(id -g)" "$public"
if [[ ! -e "$public/theme.conf" ]]; then install -m644 "$theme/theme.conf" "$public/theme.conf"; fi
[[ -L "$state/login-theme" ]] || ln -s "$public" "$state/login-theme"
install -m600 "$app_dir/../../hypr/hyprlock.conf" "$config/hyprlock.conf"
"$binary" --sync-login
sudo install -m644 "$stage/Main.qml" "$theme/Main.qml"
sudo install -m644 "$stage/Clock.qml" "$theme/components/Clock.qml"
sudo ln -sfn -- "$public/theme.conf" "$theme/theme.conf.user"
[[ $(readlink "$theme/theme.conf.user") == "$public/theme.conf" ]]
printf 'SDDM y Hyprlock vinculados. Respaldo: %s\n' "$backup"
