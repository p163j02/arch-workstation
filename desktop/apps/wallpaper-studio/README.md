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
wallpaper-studio --sync-fastfetch
```

## Fastfetch por fondo

En **Personalizar fastfetch** puedes elegir los perfiles **Volver al futuro**, **Espacio**,
**Naturaleza** y **Arch / neutro**, o **Automático según el nombre del fondo**. El automático
reconoce `bttf`/`dmc-*`, nombres espaciales como `galaxy`/`universe` y paisajes como
`paisaje`/`sunset`; para otros nombres usa Arch. No analiza el contenido de la imagen.
Puedes cambiar esa elección por fondo y compartir un perfil entre varios wallpapers.

La sección muestra la imagen y los textos del perfil. Permite elegir otra imagen, editar
el título, la frase final y las etiquetas de memoria y disco, y ajustar el ancho de la
imagen. Los campos vacíos usan los valores del perfil. **Guardar ajustes del fondo**
guarda la paleta y fastfetch sin aplicarlos; **Aplicar fondo y paleta** los guarda y aplica.
Los colores de etiquetas, título y texto siguen la paleta del fondo.

**Biblioteca…**, junto al campo de imagen, abre una galería con miniaturas de las cuatro
imágenes originales y la colección **Silphwave**: 5 de Volver al futuro, 5 de Espacio,
5 de Naturaleza y 4 de Arch. Empieza mostrando el tema del perfil elegido y permite
cambiar de tema o verlos todos. Seleccionar una miniatura cambia la imagen del borrador;
guarda o aplica para conservarla por wallpaper. Los textos y el perfil se eligen de forma
independiente. **Otra imagen…** permite buscar un archivo fuera de la biblioteca.

La colección se instala en `~/.config/fastfetch/images/silphwave/` (o en `XDG_CONFIG_HOME`).
Se vuelve a leer al abrir la biblioteca, por lo que puedes añadir más imágenes a las
subcarpetas `bttf`, `space`, `nature` y `arch`. El instalador copia las imágenes originales
del repositorio conservando sus nombres; no las recorta ni las vuelve a comprimir.

En esa misma sección, **Color del texto de los datos**, **Color de las etiquetas** y
**Color de títulos y marcos** muestran los mismos **16 colores ANSI** que aparecen en los
bloques de fastfetch: ocho normales en la primera fila y ocho brillantes en la segunda.
Todos los colores se ven como muestras antes de seleccionarlos; el color elegido queda
marcado, y al pasar el puntero sobre una muestra aparece su índice y código hexadecimal.
Hay una opción **Predeterminado** con su propia muestra: Texto para los datos, Acento para
las etiquetas y Secundario para títulos y marcos.

La paleta de 16 colores se calcula en la misma función que genera los colores de Kitty,
a partir de la paleta del fondo; no es una segunda paleta fija. Si la editas, se conserva
el índice elegido y las muestras y los textos usan su nuevo color. Cada fondo recuerda
sus elecciones. Los ajustes anteriores conservan los predeterminados y las elecciones
de Texto/Acento/Secundario/Fondo siguen siendo compatibles. **Usar valores del perfil**
también restablece estas tres elecciones.

Los ajustes se conservan por ruta del wallpaper en el estado de Studio. Aplicar o restaurar
un fondo ya configurado actualiza `~/.config/fastfetch/config.jsonc`, incluidas las llamadas
`--apply` y `--restore`. `--sync-fastfetch` actualiza solo fastfetch con el fondo guardado.
Una actualización del programa no modifica fastfetch hasta que configures un fondo desde
la interfaz. **No cambiar fastfetch** conserva la configuración que esté activa cuando se
aplique ese fondo. La salida ya impresa en las terminales no se redibuja: ejecuta fastfetch
otra vez o abre una terminal nueva.

Antes del primer reemplazo, Studio guarda una copia íntegra de la configuración anterior en
`~/.local/state/wallpaper-studio/fastfetch-backup/config.jsonc`. Para recuperar esa versión,
elige **No cambiar fastfetch** en los fondos configurados y copia el respaldo a la ruta
original. Se respetan `XDG_CONFIG_HOME` y `XDG_STATE_HOME`. El instalador del desktop copia
las cuatro imágenes de `desktop/fastfetch/images/wallpaper-studio-*.png`; se muestran con
el protocolo gráfico de Kitty, como la configuración anterior. Los ASCII quedan para una
revisión futura.

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
