# Wallpaper Studio

Gestor de fondos en **Rust + GTK4 + libadwaita** para Hyprland, incluido en el perfil autónomo del **notebook**. Ofrece imágenes estáticas y GIF animados, vista previa, búsqueda, paletas editables por fondo y transiciones con `awww`.

La paleta se aplica a los escritorios y lanzadores de Waybar, los bordes de Hyprland, el monitor de recursos Eww y los paneles del centro de control. Los fondos y las paletas personales se guardan en el equipo; no se copian del desktop a la laptop.

**Nueva extensión para la aplicación existente:** sincronización con SDDM/Pixie y Hyprlock. Para añadirla a esta copia del notebook, sigue [la guía SDDM y Hyprlock](README-SDDM-HYPRLOCK.md), con comandos de actualización, cambios y respaldos.

## Instalación en un equipo nuevo

Desde la copia actualizada de este repositorio, ejecuta como tu usuario:

```sh
bash install.sh
```

Ejecuta los comandos de esta guía desde `laptop/`.

El instalador completo instala las dependencias, compila la aplicación desde código fuente con `Cargo.lock`, crea el acceso **Wallpaper Studio** en Rofi y conecta el botón de fondos de Waybar. También configura `awww-daemon` y la restauración del fondo al iniciar sesión o cambiar monitores. Revisa los requisitos del [README del notebook](../../README.md) antes de instalar el entorno completo.

## Actualizar una laptop que ya usa Arch Workstation

```sh
cd ~/arch-workstation/laptop
git pull --ff-only
sudo pacman -Syu --needed rust pkgconf gtk4 libadwaita python python-gobject python-cairo gtk3 awww
bash apps/wallpaper-studio/install.sh
bash install.sh --config-only
bash apps/wallpaper-studio/setup-login-theme.sh
```

El comando `--config-only` actualiza **las configuraciones del perfil laptop**, conservando los ajustes locales y creando el respaldo descrito en el README principal. Revisa cualquier personalización antes de ejecutar ese paso. El último paso vincula SDDM y Hyprlock. Inicia una sesión nueva de Hyprland para cargar las integraciones actualizadas.

Si solo necesitas reinstalar los binarios y lanzadores sobre una configuración ya actualizada, basta con `bash apps/wallpaper-studio/install.sh`; ese comando no modifica la configuración activa de Waybar ni Eww.

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
```

## Inicio de sesión y bloqueo

La instalación completa del notebook vincula el tema Pixie de **SDDM** (al encender e iniciar sesión) y **Hyprlock** (el botón **Bloquear** del centro de control y Super+L). En un equipo ya instalado, después de actualizar la aplicación ejecuta una vez:

```sh
bash apps/wallpaper-studio/setup-login-theme.sh
```

Este paso necesita `sudo`, guarda un respaldo, conserva las traducciones del tema Pixie e instala la configuración de Hyprlock del repositorio. No reinicia SDDM ni cierra la sesión. Los siguientes cambios desde **Aplicar fondo y paleta** se sincronizan automáticamente, sin pedir privilegios: fondo, acento, secundario, superficies y texto.

SDDM recibe únicamente una copia PNG y los colores en `/var/lib/wallpaper-studio/<uid>/`, legibles antes del inicio de sesión; los originales y el estado personal conservan sus permisos. La última configuración aplicada por el usuario vinculado es la del inicio de sesión. Los GIF usan un fotograma estático en SDDM y Hyprlock. `--sync-login` actualiza solo estas dos pantallas con el fondo y la paleta ya guardados. Los reemplazos son atómicos por archivo; un fallo parcial se informa en la aplicación.

## Código y dependencias

Comparte el binario con Waybar Editor: el instalador coloca `~/.local/bin/waybar-editor` y el enlace `~/.local/bin/wallpaper-studio`, además de dos entradas independientes de aplicaciones. El código del gestor está en `../waybar-editor/src/wallpaper.rs` y `../waybar-editor/src/wallpaper/`. No se distribuyen binarios del desktop: cada equipo compila su copia.

Requiere Rust ≥ 1.89, GTK4 ≥ 4.10 y libadwaita ≥ 1.5. Aplicar fondos requiere `awww-daemon` activo. Esta versión usa la misma imagen en todos los monitores y admite GIF, pero no reproducción de vídeo.

La [documentación del paquete compartido](../waybar-editor/README.md) detalla los archivos de estado, los bloques de colores y la recuperación ante errores.
