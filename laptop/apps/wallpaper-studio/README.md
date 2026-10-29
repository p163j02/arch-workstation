# Wallpaper Studio

Gestor de fondos en **Rust + GTK4 + libadwaita** para Hyprland, incluido en el perfil autónomo del **notebook**. Ofrece imágenes estáticas y GIF animados, vista previa, búsqueda, paletas editables por fondo y transiciones con `awww`.

La paleta se aplica a los escritorios y lanzadores de Waybar, los bordes de Hyprland, el monitor de recursos Eww, los paneles del centro de control y la terminal Kitty. Los fondos y las paletas personales se guardan en el equipo; no se copian del desktop a la laptop.

**Nueva extensión para la aplicación existente:** sincronización con SDDM/Pixie y Hyprlock. Para añadirla a esta copia del notebook, sigue [la guía SDDM y Hyprlock](README-SDDM-HYPRLOCK.md), con comandos de actualización, cambios y respaldos.

## Instalación en un equipo nuevo

Desde la copia actualizada de este repositorio, ejecuta como tu usuario:

```sh
bash install.sh
```

Ejecuta los comandos de esta guía desde `laptop/`.

El instalador completo instala las dependencias, compila la aplicación desde código fuente con `Cargo.lock`, crea el acceso **Wallpaper Studio** en Rofi y conecta el botón de fondos de Waybar. También configura `awww-daemon` y la restauración del fondo al iniciar sesión o cambiar monitores. Revisa los requisitos del [README del notebook](../../README.md) antes de instalar el entorno completo.

## Actualizar una laptop que ya usa Arch Workstation

Cierra las ventanas de Wallpaper Studio y Waybar Editor antes de actualizar. El instalador sustituye el ejecutable de forma atómica; una ventana que siga abierta conserva su código anterior y puede volver a escribir el tema antiguo. Abre la app nuevamente después de instalarla.

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


## Llevar funcionalidades al desktop

Wallpaper Studio ya existe en ambos perfiles. Esta guía describe las funciones de la copia actual del notebook; revisa las diferencias de tu checkout antes de incorporar cambios. Los archivos de destino deben quedar en `desktop/`, manteniendo ambos perfiles independientes.

| Funcionalidad | Código del notebook | Configuración que necesita en desktop |
| --- | --- | --- |
| Fondos estáticos/GIF, carpetas, paletas y transiciones | `../waybar-editor/src/wallpaper.rs`, `src/wallpaper/palette.rs`, `src/wallpaper/theme.rs` | `awww-daemon` y el estado propio del usuario; esta base ya existe en desktop. |
| Movimiento en la vista previa grande, antes de aplicar | `../waybar-editor/src/wallpaper.rs` | Cambio de selección a `row-selected`, reloj GTK y fixture `tests/fixtures/preview.gif`. |
| Colores de Waybar y bordes de Hyprland | `../waybar-editor/src/wallpaper/theme.rs` | Bloques administrados en `~/.config/waybar/style.css` y `~/.config/hypr/local.conf`; el Lua de Hyprland debe leer este último. |
| Monitor de recursos y paneles de brillo, audio, Wi-Fi y demás | `theme.rs`, `../../eww/control-center/wallpaper-studio.scss`, scripts de Wi-Fi y sysmon | Importación del SCSS del centro de control y lectores de paleta de los widgets. Esa integración base ya está en desktop; conserva sus widgets y dispositivos. |
| Colores de la terminal Kitty | `../waybar-editor/src/wallpaper/theme.rs` | Bloque administrado en `~/.config/kitty/kitty.conf` y recarga con `SIGUSR1`. |
| Fondo y colores de SDDM/Pixie y Hyprlock | `../waybar-editor/src/wallpaper/login.rs`, llamadas en `wallpaper.rs` y `theme.rs`, `setup-login-theme.sh`, `sddm_theme.py` | Pixie instalado y seleccionado, plantilla de Hyprlock con variables y directorio público por usuario. |

Para incorporar la versión completa de Wallpaper Studio del notebook, los archivos de código que difieren actualmente son `wallpaper.rs`, `wallpaper/theme.rs` y el nuevo `wallpaper/login.rs`; añade también el fixture de GIF para poder ejecutar sus pruebas. No hace falta copiar todo Waybar Editor ni sus configuraciones de escritorio.

Si solo quieres la animación de GIF, adapta los cambios de selección, `TickCallbackId`, cancelación del callback y prueba de animación de `wallpaper.rs`; ese archivo también contiene los cambios de inicio/bloqueo, por lo que copiarlo entero exige incorporar `login.rs` y su integración en `theme.rs`.

Para **paleta y fondo de SDDM/Hyprlock**, sigue [el procedimiento de traslado al desktop](README-SDDM-HYPRLOCK.md#incorporar-esta-funcionalidad-al-desktop). Incluye las rutas exactas, la plantilla de bloqueo, compilación, activación, comprobaciones y recuperación.

Mantén las imágenes, carpetas, paletas y monitores del desktop. No copies `laptop/state/`, `laptop/waybar/` ni todo `laptop/hypr/` para habilitar una función. Ambos perfiles usan el estado del usuario que ejecuta la app, no una carpeta de estado compartida dentro del repositorio.


## Paleta de la terminal Kitty

El patrón se calcula para **todos los fondos**, a partir de su paleta actual; no contiene una excepción ni colores fijos para `paisaje.jpg`. **Aplicar fondo y paleta**, `--apply` y `--restore` también actualizan Kitty cuando existe `~/.config/kitty/kitty.conf` (o su equivalente bajo `XDG_CONFIG_HOME`). **Guardar paleta** solo guarda el borrador; `--sync-login` sigue limitado a SDDM/Hyprlock.

La app administra un bloque `# wallpaper-studio:begin` / `# wallpaper-studio:end` al final de `kitty.conf`: fondo, texto, cursor, selección, pestañas, bordes y los 16 colores ANSI. Usa el mismo cálculo de tonos de la copia desktop: los dos acentos generan azul y magenta; el cian mezcla ambos. Rojo, verde y amarillo se tiñen suavemente conservando su significado. Los ocho colores claros se obtienen mezclando los tonos normales con el texto, para evitar repeticiones. El gris de comentarios y las superficies se derivan del fondo y el texto. Las fuentes, transparencia, atajos y demás ajustes quedan fuera del bloque y se conservan.

La configuración se guarda para nuevas ventanas y se solicita la recarga de las abiertas mediante `SIGUSR1`, según la [documentación de Kitty](https://sw.kovidgoyal.net/kitty/conf/). El cursor y la selección calculan texto blanco o negro según el contraste del acento. No necesita habilitar control remoto. Si Kitty no está abierto, el cambio queda preparado; si falta su configuración, no se crea una nueva. Un fallo al aplicar el fondo restaura el archivo anterior. Si falla únicamente la recarga, se muestra una advertencia y los colores quedan guardados.

Para incorporar esta función al desktop, adapta `Paths.kitty`, `themed_kitty` y la preparación, escritura, reversión y recarga de `apply_with` en `../waybar-editor/src/wallpaper/theme.rs`. Añade el campo `kitty` a los fixtures que construyen `Paths`, incluido el de `wallpaper/login.rs`, y traslada las pruebas de tema/recarga/reversión. Si ya incorporaste la integración SDDM, no necesitas volver a preparar Pixie. Recompila e instala desde `desktop/apps/`, conservando su propio `kitty.conf`; el bloque se genera al aplicar su paleta.

Para recuperar los colores anteriores, retira solo el bloque administrado y recarga Kitty. Las aplicaciones que definen su propio tema con colores RGB, como algunos editores, pueden conservar esos colores propios.


La regresión `paisaje_ansi_palette_matches_desktop_reference` verifica los 16 tonos para la paleta de `paisaje.jpg` usada en la comparación. La lógica de `mix` y `themed_kitty` corresponde a `mix` y `themed_config` en `desktop/apps/waybar-editor/src/wallpaper/terminal.rs` de la revisión `f74e33f`; cada perfil conserva su propia implementación. El notebook mantiene su preparación y reversión del archivo dentro de la aplicación del fondo.
