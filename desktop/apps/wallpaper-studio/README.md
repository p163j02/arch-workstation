# Wallpaper Studio

Gestor de fondos en **Rust + GTK4 + libadwaita** para Hyprland del perfil **desktop** de Arch Workstation. Ofrece imágenes estáticas y GIF animados, vista previa, búsqueda, paletas editables por fondo y transiciones con `awww`. El perfil laptop mantiene su [propia versión y documentación](../../../laptop/apps/wallpaper-studio/README.md).

La paleta se aplica a los escritorios y lanzadores de Waybar, los bordes de Hyprland, el monitor de recursos Eww, los paneles del centro de control y la terminal Kitty. También sincroniza el fondo y los colores de SDDM/Pixie y Hyprlock después de preparar la integración. Los fondos y las paletas personales se guardan en el equipo; no se copian del desktop a la laptop.

Los bordes activos mantienen un gradiente de acento a secundario a 45°. En Hyprland 0.57 se actualizan en vivo mediante `hyprctl eval` y `hl.config`; el bloque de `hypr/local.conf` conserva la paleta al recargar la configuración.

## Instalación en un equipo nuevo

Desde la copia actualizada de este repositorio, ejecuta como tu usuario:

```sh
bash install.sh desktop
```

El instalador completo instala las dependencias, compila la aplicación desde código fuente con `Cargo.lock`, crea el acceso **Wallpaper Studio** en Rofi y conecta el botón de fondos de Waybar. También configura `awww-daemon`, la restauración del fondo al iniciar sesión o cambiar monitores y la integración con SDDM/Pixie y Hyprlock. Revisa los requisitos del [README principal](../../../README.md) antes de instalar el entorno completo.

## Actualizar un desktop que ya usa Arch Workstation

```sh
cd ~/arch-workstation
git pull --ff-only
sudo pacman -Syu --needed rust pkgconf gtk4 libadwaita python python-gobject python-cairo gtk3 awww
bash desktop/apps/wallpaper-studio/install.sh
bash install.sh desktop --config-only
```

El último comando actualiza **las configuraciones del perfil desktop**, conservando los ajustes locales y creando el respaldo descrito en el README principal. Revisa cualquier personalización antes de ejecutar ese paso. Inicia una sesión nueva de Hyprland para cargar las integraciones actualizadas.

Si solo necesitas reinstalar los binarios y lanzadores sobre una configuración ya actualizada, basta con `bash desktop/apps/wallpaper-studio/install.sh`; ese comando no modifica la configuración activa de Waybar ni Eww.

## Configuración integrada

También está integrado en [Configuración del PC](../pc-settings/README.md), junto a Waybar Editor, pantallas y ajustes de transparencia/widgets. Abre `pc-settings`; el acceso independiente `wallpaper-studio` sigue disponible. Cambiar de fondo conserva las transparencias configuradas desde la nueva app.

## Abrir y usar

```sh
wallpaper-studio
```

Elige un fondo, revisa o edita la paleta y pulsa **Aplicar fondo y paleta**. **Extraer de nuevo** recalcula los colores; las paletas que hayas guardado se respetan hasta que las cambies. El programa también admite:

```sh
wallpaper-studio --help
wallpaper-studio --apply /ruta/al/fondo.gif
wallpaper-studio --restore
wallpaper-studio --sync-login
wallpaper-studio --sync-terminal
```

## Colores de la terminal

En desktop, **Aplicar fondo y paleta** también actualiza Kitty: fondo, texto, cursor, selección, pestañas, bordes y los 16 colores ANSI. Los rojos, verdes y amarillos mantienen su significado para errores, confirmaciones y avisos. Conserva la fuente, transparencia, márgenes y atajos existentes.

La app añade o actualiza un bloque `# wallpaper-studio:begin` / `# wallpaper-studio:end` al final de `~/.config/kitty/kitty.conf` y solicita una recarga con `SIGUSR1`. Las ventanas abiertas y las nuevas usan los colores guardados, sin cerrar sus procesos. Si Kitty aún no tiene configuración, no crea una. Con `XDG_CONFIG_HOME` personalizado utiliza ese directorio.

Para sincronizar solo la terminal con la selección guardada, ejecuta `wallpaper-studio --sync-terminal`. Para volver a tus colores anteriores, retira ese bloque administrado y recarga Kitty con Ctrl+Shift+F5.

## Fondo y colores del acceso y del bloqueo

En un desktop ya instalado, actualiza el binario, aplica un fondo en la app y ejecuta:

```sh
bash desktop/apps/wallpaper-studio/setup-login-theme.sh
wallpaper-studio --sync-login
```

La preparación requiere `sudo` una vez, Pixie instalado y seleccionado en SDDM. Respalda el tema instalado y `~/.config/hypr/hyprlock.conf` en un directorio `login-backup.XXXXXXXX/` dentro del estado de Wallpaper Studio. Sustituye la configuración activa del bloqueo por `desktop/hypr/hyprlock.conf`, conservando el diseño del desktop. No reinicia SDDM ni cierra la sesión.

Después, **Aplicar fondo y paleta** actualiza ambas pantallas como usuario normal. SDDM lee únicamente `background.png` y `theme.conf`, exportados con permisos de lectura en `/var/lib/wallpaper-studio/<uid>/`; los originales y el estado personal conservan sus permisos. Los GIF usan un fotograma estático en el acceso y el bloqueo. `--sync-login` resincroniza solo estas pantallas con la selección guardada.

Para comprobarlo, revisa `~/.local/state/wallpaper-studio/login-theme` y `/usr/share/sddm/themes/pixie/theme.conf.user`: deben enlazar al directorio público y a su `theme.conf`. El bloque administrado al principio de Hyprlock debe apuntar al PNG exportado. Si reinstalas la plantilla de Hyprlock, vuelve a ejecutar la preparación.

Para recuperar la configuración anterior, restaura `hyprlock.conf` desde el respaldo y, con `sudo`, `Main.qml` y `Clock.qml` en Pixie. Retira el enlace `theme.conf.user` y restaura la copia anterior si existía. Conserva el respaldo hasta verificar un inicio y desbloqueo reales.

## Código y dependencias

Comparte el binario con Waybar Editor: el instalador coloca `~/.local/bin/waybar-editor` y el enlace `~/.local/bin/wallpaper-studio`, además de dos entradas independientes de aplicaciones. El código del gestor está en `../waybar-editor/src/wallpaper.rs` y `../waybar-editor/src/wallpaper/`. No se distribuyen binarios del desktop: cada equipo compila su copia.

Requiere Rust ≥ 1.89, GTK4 ≥ 4.10 y libadwaita ≥ 1.5. Aplicar fondos requiere `awww-daemon` activo. Esta versión usa la misma imagen en todos los monitores y admite GIF, pero no reproducción de vídeo.

La [documentación del paquete compartido](../waybar-editor/README.md) detalla los archivos de estado, los bloques de colores y la recuperación ante errores.
