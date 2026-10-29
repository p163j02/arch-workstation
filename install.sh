#!/usr/bin/env bash
set -Eeuo pipefail
trap 'echo "Error en la línea $LINENO. La instalación no terminó." >&2' ERR
if [[ ${1:-} == laptop ]]; then
    shift
    exec bash "$(dirname -- "${BASH_SOURCE[0]}")/laptop/install.sh" "$@"
fi
source "$(dirname -- "${BASH_SOURCE[0]}")/scripts/common.sh"
require_profile "${1:-}"
MODE="${2:---install}"
case "$MODE" in --install|--check|--config-only) ;; *) echo "Opción inválida: $MODE" >&2; exit 2 ;; esac
[[ $# -le 2 ]] || { echo "Demasiados argumentos." >&2; exit 2; }

if [[ "$MODE" == --check ]]; then
    exec python3 "$REPO_DIR/scripts/check.py" --packages
fi

[[ $EUID -ne 0 ]] || { echo "Ejecuta como tu usuario, sin sudo." >&2; exit 1; }
[[ "$CONFIG_DIR" == "$HOME/.config" ]] || {
    echo "Estas configuraciones requieren XDG_CONFIG_HOME=$HOME/.config." >&2; exit 1;
}
# The kernel shebang and several desktop-entry formats require a conventional home path.
[[ "$HOME" =~ ^/[a-zA-Z0-9_./-]+$ ]] || {
    echo "HOME debe ser una ruta absoluta sin espacios ni caracteres especiales." >&2; exit 1;
}

if [[ "$MODE" == --install ]]; then
    [[ -f /etc/arch-release ]] || { echo "Este instalador requiere Arch Linux." >&2; exit 1; }
    command -v sudo >/dev/null || { echo "Instala sudo y habilítalo para tu usuario." >&2; exit 1; }
    pacman-conf --repo-list | grep -Fxq multilib || {
        echo "Habilita [multilib] en /etc/pacman.conf; ver README.md." >&2; exit 1;
    }
fi

echo "Perfil: $PROFILE. Se respaldarán los archivos existentes antes de reemplazarlos."
read -r -p "Escribe si para continuar: " confirm
[[ "$confirm" == si ]] || { echo "Cancelado."; exit 1; }

BUILD_DIR="$(mktemp -d -t dotfiles-install.XXXXXXXX)"
trap 'rm -rf -- "$BUILD_DIR"' EXIT

if [[ "$MODE" == --install ]]; then
    package_list="$(read_packages "$REPO_DIR/$PROFILE/packages/official.txt")"
    mapfile -t packages <<< "$package_list"
    # Install headers for every supported installed kernel; do not change the bootloader.
    mapfile -t kernels < <(pacman -Qq | grep -Ex 'linux|linux-lts|linux-zen|linux-hardened' || true)
    [[ ${#kernels[@]} -gt 0 ]] || {
        echo "Se requiere un kernel instalado: linux, linux-lts, linux-zen o linux-hardened." >&2; exit 1;
    }
    for kernel in "${kernels[@]}"; do packages+=("$kernel-headers"); done
    hardware_list="$(python3 "$REPO_DIR/scripts/hardware.py")"
    if [[ -n "$hardware_list" ]]; then
        mapfile -t hardware <<< "$hardware_list"
        packages+=("${hardware[@]}")
    fi
    sudo pacman -Syu --needed "${packages[@]}"
    python3 "$REPO_DIR/scripts/check.py" --packages

    if ! command -v yay >/dev/null; then
        git clone --depth 1 https://aur.archlinux.org/yay.git "$BUILD_DIR/yay"
        (cd "$BUILD_DIR/yay" && makepkg -si)
    fi
    mapfile -t aur < <(read_packages "$REPO_DIR/$PROFILE/packages/aur.txt")
    yay -S --needed "${aur[@]}"
    sudo corepack enable
    corepack prepare pnpm@latest --activate

    # Each profile owns its source; both applications use its locked Rust build.
    bash "$REPO_DIR/$PROFILE/apps/wallpaper-studio/install.sh"

    if [[ "$PROFILE" == laptop && ! -x "$HOME/.local/bin/airpods-tui" ]]; then
        # The panel uses the IPC protocol verified with this exact upstream revision.
        cargo install --locked --git https://github.com/annoyedmilk/airpods-tui \
            --rev 8af4eb20cbc39c33983d70c0518753db13f0bbfa --root "$HOME/.local" airpods-tui
    fi
fi

for command in rsync python3 cc; do
    command -v "$command" >/dev/null || { echo "Falta $command; ver README.md." >&2; exit 1; }
done
python3 "$REPO_DIR/scripts/check.py"

STAGE="$BUILD_DIR/config"
mkdir -p "$STAGE"
for name in "${CONFIG_NAMES[@]}"; do
    rsync -a "${RSYNC_EXCLUDES[@]}" --exclude='*.so' "$REPO_DIR/$PROFILE/$name/" "$STAGE/$name/"
done
if [[ -d "$REPO_DIR/$PROFILE/nwg-displays" ]]; then
    rsync -a "$REPO_DIR/$PROFILE/nwg-displays/" "$STAGE/nwg-displays/"
fi
python3 "$REPO_DIR/scripts/portable.py" render "$STAGE" "$HOME"
cc -O2 -Wall -Wextra "$STAGE/eww/control-center/scripts/fast-client.c" \
    -o "$STAGE/eww/control-center/scripts/fast-client"
for name in backend.py popup-manager.py; do
    sed -i "1c#!$CONFIG_DIR/eww/control-center/scripts/fast-client" "$STAGE/eww/control-center/scripts/$name"
done
if [[ -f "$STAGE/waybar/cffi/build-animation.py" ]]; then
    python3 "$STAGE/waybar/cffi/build-animation.py" --install-directory "$CONFIG_DIR/waybar/cffi"
fi

BACKUP="${XDG_STATE_HOME:-$HOME/.local/state}/dotfiles/backups/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p -m700 "$BACKUP"
chmod 700 "$BACKUP"
mkdir -p "$BACKUP/config" "$CONFIG_DIR"
install_tree() {
    local name="$1"
    [[ -d "$STAGE/$name" ]] || { echo "Falta la configuración $name" >&2; return 1; }
    if [[ -e "$CONFIG_DIR/$name" || -L "$CONFIG_DIR/$name" ]]; then
        cp -a -- "$CONFIG_DIR/$name" "$BACKUP/config/$name"
        [[ ! -L "$CONFIG_DIR/$name" ]] || { echo "Retira el enlace $CONFIG_DIR/$name antes de instalar." >&2; return 1; }
    fi
    mkdir -p "$CONFIG_DIR/$name"
    rsync -a --delete "${RSYNC_EXCLUDES[@]}" --exclude=local.conf --exclude=local.lua "$STAGE/$name/" "$CONFIG_DIR/$name/"
}
for name in "${CONFIG_NAMES[@]}"; do install_tree "$name"; done
install -m755 "$STAGE/eww/control-center/scripts/fast-client" "$CONFIG_DIR/eww/control-center/scripts/fast-client"
[[ ! -d "$STAGE/nwg-displays" ]] || install_tree nwg-displays
[[ -e "$HOME/.nanorc" ]] && cp -a "$HOME/.nanorc" "$BACKUP/nanorc"
install -m644 "$REPO_DIR/$PROFILE/nano/user.nanorc" "$HOME/.nanorc"
touch "$CONFIG_DIR/hypr/local.conf"
find "$CONFIG_DIR/hypr" "$CONFIG_DIR/waybar" "$CONFIG_DIR/eww" -type f \
    \( -name '*.sh' -o -name '*.py' \) -exec chmod +x {} +

mkdir -p "$HOME/wallpapers" "$CONFIG_DIR/dotfiles" "$CONFIG_DIR/systemd/user"
rsync -a --no-owner --no-group --backup --backup-dir="$BACKUP/wallpapers" "$REPO_DIR/$PROFILE/wallpapers/" "$HOME/wallpapers/"
[[ ! -e "$CONFIG_DIR/dotfiles/profile" ]] || cp "$CONFIG_DIR/dotfiles/profile" "$BACKUP/profile"
printf '%s\n' "$PROFILE" > "$CONFIG_DIR/dotfiles/profile"
for unit in "$REPO_DIR/$PROFILE/systemd/user/"*.service; do
    [[ ! -e "$CONFIG_DIR/systemd/user/${unit##*/}" ]] || cp -a "$CONFIG_DIR/systemd/user/${unit##*/}" "$BACKUP/"
    cp "$unit" "$CONFIG_DIR/systemd/user/"
done
if [[ "$PROFILE" == laptop ]]; then
    mkdir -p "$CONFIG_DIR/airpods-tui"
    if [[ ! -f "$CONFIG_DIR/airpods-tui/config.toml" ]]; then
        printf 'ble_scan = false\nauto_connect = false\nvolume_osd_command = []\n' > "$CONFIG_DIR/airpods-tui/config.toml"
    fi
fi

if systemctl --user show-environment >/dev/null 2>&1; then
    systemctl --user daemon-reload
fi

if [[ "$MODE" == --install ]]; then
    # Preserve system files before deploying this profile's login and memory configuration.
    for path in /etc/sddm.conf.d/theme.conf /etc/systemd/zram-generator.conf \
        /etc/sysctl.d/99-swappiness.conf; do
        if sudo test -e "$path"; then sudo cp --parents -a "$path" "$BACKUP/"; fi
    done
    sudo install -Dm644 "$REPO_DIR/$PROFILE/sddm/theme.conf" /etc/sddm.conf.d/theme.conf
    sudo mkdir -p /usr/share/sddm/themes/pixie
    if sudo test -d /usr/share/sddm/themes/pixie; then
        sudo cp -a /usr/share/sddm/themes/pixie "$BACKUP/pixie-theme"
    fi
    sudo rsync -a --chown=root:root "$REPO_DIR/$PROFILE/sddm/pixie/" /usr/share/sddm/themes/pixie/
    sudo install -Dm644 "$REPO_DIR/$PROFILE/system/zram-generator.conf" /etc/systemd/zram-generator.conf
    sudo install -Dm644 "$REPO_DIR/$PROFILE/system/99-swappiness.conf" /etc/sysctl.d/99-swappiness.conf
    sudo systemctl daemon-reload
    sudo systemctl enable NetworkManager sddm firewalld docker power-profiles-daemon
    [[ "$PROFILE" != laptop ]] || sudo systemctl enable bluetooth
    # Do not restart networking, SDDM or Bluetooth in the user's active session.
    sudo usermod -aG docker,vboxusers "$(id -un)"
    [[ "${SHELL:-}" == */fish ]] || chsh -s /usr/bin/fish

    # GTK en Wayland toma estos valores de GSettings, incluso si settings.ini existe.
    python3 "$REPO_DIR/scripts/gtk-settings.py" "$REPO_DIR/$PROFILE/system/desktop-interface.ini"
fi

echo "Configuración instalada. Respaldo: $BACKUP"
echo "Reinicia para aplicar los servicios y la pertenencia a grupos."
