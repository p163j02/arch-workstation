#!/bin/sh
set -eu
cd "$(dirname "$0")"
for dependency in cargo pkg-config; do
    command -v "$dependency" >/dev/null 2>&1 || {
        echo "Falta $dependency; instala rust pkgconf gtk4 libadwaita." >&2
        exit 1
    }
done
pkg-config --atleast-version=4.10 gtk4 || { echo "Se requiere GTK4 >= 4.10." >&2; exit 1; }
pkg-config --atleast-version=1.5 libadwaita-1 || { echo "Se requiere libadwaita >= 1.5." >&2; exit 1; }
cargo build --release --locked --target-dir target
binary="$HOME/.local/bin/waybar-editor"
wallpaper_binary="$HOME/.local/bin/wallpaper-studio"
entry="${XDG_DATA_HOME:-$HOME/.local/share}/applications/waybar-editor.desktop"
# Replace the executable atomically so an already open editor can keep running.
mkdir -p "$(dirname "$binary")"
binary_temp="$(mktemp "$(dirname "$binary")/.waybar-editor.XXXXXXXX")"
trap 'rm -f -- "$binary_temp"' EXIT
install -m755 target/release/waybar-editor "$binary_temp"
mv -f -- "$binary_temp" "$binary"
trap - EXIT
ln -sfn waybar-editor "$wallpaper_binary"
mkdir -p "$(dirname "$entry")"
printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=Waybar Editor' 'Comment=Edita tu barra, aplicaciones y presets de Waybar' "Exec=\"$binary\"" 'Icon=preferences-desktop' 'Terminal=false' 'Categories=Settings;DesktopSettings;' 'Keywords=Waybar;Hyprland;Barra;Panel;' 'StartupWMClass=io.local.WaybarEditor' > "$entry"
chmod 644 "$entry"
wallpaper_entry="${XDG_DATA_HOME:-$HOME/.local/share}/applications/wallpaper-studio.desktop"
printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=Wallpaper Studio' 'Comment=Fondos animados y paletas dinámicas para Hyprland' "Exec=\"$wallpaper_binary\"" 'Icon=preferences-desktop-wallpaper' 'Terminal=false' 'Categories=Settings;DesktopSettings;' 'Keywords=Wallpaper;Fondo;GIF;Paleta;Hyprland;' 'StartupWMClass=io.local.WallpaperStudio' > "$wallpaper_entry"
chmod 644 "$wallpaper_entry"
if command -v desktop-file-validate >/dev/null 2>&1; then desktop-file-validate "$entry"; desktop-file-validate "$wallpaper_entry"; fi
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$(dirname "$entry")"; fi
printf 'Instalado: %s\nInstalado: %s\nLanzadores: %s, %s\n' "$binary" "$wallpaper_binary" "$entry" "$wallpaper_entry"
