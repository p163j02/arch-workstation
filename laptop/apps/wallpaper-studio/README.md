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
wallpaper-studio --sync-terminal
wallpaper-studio --sync-fastfetch
```

## Inicio de sesión y bloqueo

La instalación completa del notebook vincula el tema Pixie de **SDDM** (al encender e iniciar sesión) y **Hyprlock** (el botón **Bloquear** del centro de control y Super+L). En un equipo ya instalado, después de actualizar la aplicación ejecuta una vez:

```sh
bash apps/wallpaper-studio/setup-login-theme.sh
```

Este paso necesita `sudo`, guarda un respaldo, conserva las traducciones del tema Pixie e instala la configuración de Hyprlock del repositorio. No reinicia SDDM ni cierra la sesión. Los siguientes cambios desde **Aplicar fondo y paleta** se sincronizan automáticamente, sin pedir privilegios: fondo, acento, secundario, superficies y texto.

SDDM recibe únicamente una copia PNG y los colores en `/var/lib/wallpaper-studio/<uid>/`, legibles antes del inicio de sesión; los originales y el estado personal conservan sus permisos. La última configuración aplicada por el usuario vinculado es la del inicio de sesión. Los GIF usan un fotograma estático en SDDM y Hyprlock. `--sync-login` actualiza solo estas dos pantallas con el fondo y la paleta ya guardados. Los reemplazos son atómicos por archivo; un fallo parcial se informa en la aplicación.

## Código y dependencias

Comparte el binario con Waybar Editor: el instalador coloca `~/.local/bin/waybar-editor` y el enlace `~/.local/bin/wallpaper-studio`, además del enlace `~/.local/bin/pc-settings` y tres entradas independientes de aplicaciones. El código del gestor está en `../waybar-editor/src/wallpaper.rs` y `../waybar-editor/src/wallpaper/`. No se distribuyen binarios del desktop: cada equipo compila su copia.

Requiere Rust ≥ 1.89, GTK4 ≥ 4.10 y libadwaita ≥ 1.5. Aplicar fondos requiere `awww-daemon` activo. Esta versión usa la misma imagen en todos los monitores y admite GIF, pero no reproducción de vídeo.

La [documentación del paquete compartido](../waybar-editor/README.md) detalla los archivos de estado, los bloques de colores y la recuperación ante errores.


## Configuración integrada

Abre `pc-settings` o **Configuración del notebook** desde Rofi para usar Wallpaper Studio,
Waybar Editor, pantallas y ajustes de widgets en una sola ventana. Los accesos independientes
siguen disponibles. Consulta [la guía de Configuración](../pc-settings/README.md).

## Fastfetch por fondo

En **Personalizar fastfetch**, elige **Automático**, **Volver al futuro**, **Espacio**,
**Naturaleza**, **Arch / neutro** o **No cambiar fastfetch**. Puedes personalizar imagen,
ancho, título, frase final y etiquetas de memoria/disco. Los tres selectores de color
muestran los mismos 16 colores ANSI que Kitty.

**Biblioteca…** ofrece cuatro imágenes originales y 19 variantes Silphwave, agrupadas por
tema. **Otra imagen…** permite seleccionar un archivo propio. Las imágenes se incluyen en
`laptop/fastfetch/images/` y el instalador de la app las copia a `~/.config/fastfetch/images/`.

**Guardar ajustes del fondo** conserva el borrador por wallpaper. **Aplicar fondo y paleta**
guarda y actualiza Fastfetch; el resultado se ve al ejecutarlo de nuevo. El primer cambio
respalda la configuración anterior junto al estado de Wallpaper Studio, en
`fastfetch-backup/config.jsonc`. Si el fondo no tiene ajustes guardados, `--restore` conserva
Fastfetch como está. Los ajustes anteriores de paletas y fondos siguen siendo compatibles.

La referencia de implementación es `desktop/`; todos los cambios y recursos del notebook
se mantienen dentro de `laptop/`, sin dependencias de ejecución entre perfiles.

## Paleta de la terminal Kitty

El patrón se calcula para **todos los fondos**, a partir de su paleta actual; no contiene una excepción ni colores fijos para `paisaje.jpg`. **Aplicar fondo y paleta**, `--apply` y `--restore` también actualizan Kitty cuando existe `~/.config/kitty/kitty.conf` (o su equivalente bajo `XDG_CONFIG_HOME`). **Guardar ajustes del fondo** solo guarda el borrador; `--sync-login` sigue limitado a SDDM/Hyprlock.

La app administra un bloque `# wallpaper-studio:begin` / `# wallpaper-studio:end` al final de `kitty.conf`: fondo, texto, cursor, selección, pestañas, bordes y los 16 colores ANSI. Usa el mismo cálculo de tonos de la copia desktop: los dos acentos generan azul y magenta; el cian mezcla ambos. Rojo, verde y amarillo se tiñen suavemente conservando su significado. Los ocho colores claros se obtienen mezclando los tonos normales con el texto, para evitar repeticiones. El gris de comentarios y las superficies se derivan del fondo y el texto. Las fuentes, transparencia, atajos y demás ajustes quedan fuera del bloque y se conservan.

La configuración se guarda para nuevas ventanas y se solicita la recarga de las abiertas mediante `SIGUSR1`, según la [documentación de Kitty](https://sw.kovidgoyal.net/kitty/conf/). El cursor y la selección calculan texto blanco o negro según el contraste del acento. No necesita habilitar control remoto. Si Kitty no está abierto, el cambio queda preparado; si falta su configuración, no se crea una nueva. Kitty se sincroniza después de aplicar correctamente el fondo; si falla el backend del fondo, su archivo queda intacto. Si falla únicamente la recarga, se muestra una advertencia y los colores quedan guardados.

Para recuperar los colores anteriores, retira solo el bloque administrado y recarga Kitty. Las aplicaciones que definen su propio tema con colores RGB, como algunos editores, pueden conservar esos colores propios.


La regresión `paisaje_ansi_palette_matches_desktop_reference` verifica los 16 tonos para la paleta de `paisaje.jpg` usada en la comparación. El cálculo compartido por Kitty y los selectores de Fastfetch está en `../waybar-editor/src/wallpaper/terminal.rs`. Cada perfil conserva su propia copia.
