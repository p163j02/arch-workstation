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
entry="${XDG_DATA_HOME:-$HOME/.local/share}/applications/waybar-editor.desktop"
install -Dm755 target/release/waybar-editor "$binary"
mkdir -p "$(dirname "$entry")"
printf '%s\n' '[Desktop Entry]' 'Type=Application' 'Name=Waybar Editor' 'Comment=Edita tu barra, aplicaciones y presets de Waybar' "Exec=\"$binary\"" 'Icon=preferences-desktop' 'Terminal=false' 'Categories=Settings;DesktopSettings;' 'Keywords=Waybar;Hyprland;Barra;Panel;' 'StartupWMClass=io.local.WaybarEditor' > "$entry"
chmod 644 "$entry"
if command -v desktop-file-validate >/dev/null 2>&1; then desktop-file-validate "$entry"; fi
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$(dirname "$entry")"; fi
printf 'Instalado: %s\nLanzador: %s\n' "$binary" "$entry"
