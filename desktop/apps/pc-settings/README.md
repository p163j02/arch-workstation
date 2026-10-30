# Configuración del PC

App nativa en **Rust + GTK4 + libadwaita** para el perfil desktop. Integra las interfaces completas de Wallpaper Studio y Waybar Editor dentro de una sola ventana, con módulos nuevos de pantallas y de barra/widgets. Los accesos independientes anteriores siguen disponibles. Laptop conserva su propia versión de las apps y sus configuraciones.

## Instalar y abrir

```sh
cd ~/arch-workstation
bash desktop/apps/pc-settings/install.sh
pc-settings
```

También aparece como **Configuración del PC** en Rofi. El instalador del perfil desktop ya la incluye en las instalaciones completas. El código comparte el paquete y `Cargo.lock` de `../waybar-editor`; el nuevo lanzador selecciona la ventana integrada. No necesita dependencias adicionales a las apps existentes ni permisos de administrador.

El instalador de la app actualiza binarios y accesos. En una instalación anterior también hay que actualizar las configuraciones del perfil desktop para conectar Eww y los adaptadores de los paneles. `bash install.sh desktop --config-only` hace esa actualización y crea un respaldo de las configuraciones, conservando `hypr/local.conf` y los ajustes privados. Revisa tus personalizaciones antes de actualizar todo el perfil. Los cambios nuevos de esta integración afectan `eww/desktop/sysmon/eww.yuck`, `hypr/local-settings.lua`, `hypr/session-start.sh` y el adaptador existente del reproductor.

## Pantallas

- **Volver a detectar** consulta las pantallas conectadas, incluidas las desactivadas, mediante `hyprctl -j monitors all`.
- Cada pantalla ofrece encendido, resolución, frecuencias válidas para esa resolución, posición X/Y en píxeles lógicos y escala. Por ejemplo, 120 Hz solo aparece si el monitor anuncia ese modo en la resolución elegida; se conservan frecuencias fraccionarias como 59,940 Hz. No se fabrican modos ni se fuerza un límite genérico de 120 Hz.
- Conserva las rotaciones y espejos existentes. Esta versión no crea nuevos espejos ni cambia la rotación desde la interfaz.
- **Probar cambios** valida de nuevo las conexiones, impide apagar todas las pantallas y activa los destinos antes de desactivar otros monitores. Comprueba que Hyprland haya aceptado el modo y la escala.
- **Conservar** guarda únicamente un bloque delimitado en `hypr/local.conf`. Conserva el resto del archivo y los ajustes guardados de monitores temporalmente desconectados. Las configuraciones Lua del perfil cargan ese archivo mediante `local-settings.lua`.
- Sin confirmación, el diálogo restaura los ajustes anteriores a los 15 segundos. Un proceso Rust independiente restaura a los 25 segundos desde el comienzo de la prueba si la interfaz se interrumpe. El cambio persistente se escribe solo al confirmar. Un corte completo de sesión o del equipo antes de confirmar vuelve a la configuración previamente guardada al iniciar de nuevo.

La selección de modos sigue la [documentación oficial de Hyprland](https://wiki.hypr.land/configuring/core/monitors/modes/). Las operaciones en vivo utilizan `hyprctl eval` y `hl.monitor`, compatibles con la configuración Lua del desktop.

## Barra y widgets

**Transparencia** usa 0 % para un fondo opaco y 100 % para un fondo transparente. Conserva la opacidad de letras e iconos. Hay controles independientes para Waybar, audio, Wi-Fi, Bluetooth, AirPods, brillo, calendario, energía, luz nocturna, apagado, reproductor y monitor de recursos.

El monitor de recursos permite elegir nueve anclajes y desplazamientos horizontal/vertical. Se aplica a las instancias de Eww abiertas (`sysmon-widget` y `sysmon-widget-ext`). Permite mostrar u ocultar CPU/frecuencia, RAM, disco/E/S, temperatura de CPU, temperatura de GPU, Top CPU y Top RAM. Las temperaturas sin sensor disponible muestran `—`; ocultar una sección no desactiva sus recolectores existentes.

**Aplicar barra y widgets** guarda los valores, actualiza CSS, solicita la recarga de Waybar y de Eww y reabre las instancias activas del monitor de recursos para actualizar su geometría. El inicio de sesión utiliza `pc-settings --open-sysmon` para recuperar la posición guardada; una apertura manual con `eww open` utiliza la geometría predeterminada de Eww. Los paneles nativos de Wi-Fi y del reproductor observan el tema compartido. Los pequeños adaptadores ya existentes conservan su lenguaje; la nueva interfaz, persistencia, aplicación de transparencia y gestión de pantallas están implementadas en Rust.

Cambiar el wallpaper conserva estas transparencias. Waybar Editor también conserva el control independiente de transparencia al editar su apariencia. Los presets de Waybar incluyen su propio bloque de transparencia cuando existe. Si cambias la barra desde otra sección o desde la app independiente y el editor tiene un borrador abierto, el editor detecta el cambio externo al aplicar; guarda el borrador como preset y recarga sus archivos.

## Persistencia y recuperación

Con los directorios XDG predeterminados:

- `~/.config/pc-settings/widgets.json`: transparencias, posición y secciones visibles. Se valida antes de guardar y al leer. Abrir la app no escribe valores predeterminados sobre tu configuración.
- `~/.config/waybar/style.css`: bloque `/* pc-settings:bar:begin */` / `end`, con la transparencia de la barra y sus metadatos.
- `~/.config/eww/control-center/wallpaper-studio.scss`: bloque `/* pc-settings:panels:begin */` / `end`, con los fondos de cada desplegable.
- `~/.config/hypr/local.conf`: bloque `# pc-settings:monitors:begin` / `end`, con los monitores confirmados.
- `~/.local/state/pc-settings/backups/<id>/files.json`: rutas y contenidos anteriores a cada guardado, incluyendo `null` para archivos que no existían.

Respeta `XDG_CONFIG_HOME` y `XDG_STATE_HOME`. Los archivos se reemplazan atómicamente por separado, con permisos `0600`, respaldo previo y comprobación de cambios externos. Si una escritura falla se intentan restaurar las anteriores; un cierre forzado entre archivos puede dejar una actualización parcial. Los guardados de las apps comparten un bloqueo para evitar sobrescribir sus cambios concurrentes. No se sustituyen archivos activos que sean enlaces simbólicos.

Para recuperar una versión anterior, revisa el manifiesto del respaldo y restaura los contenidos en sus rutas; después recarga Waybar/Eww o Hyprland según el archivo. Conserva los respaldos hasta comprobar el resultado. No borres los bloques de colores de Wallpaper Studio para retirar únicamente estos ajustes.

## Validar

```sh
pc-settings --help
pc-settings --check  # lectura: requiere la sesión de Hyprland
cargo test --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml
cargo clippy --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml --all-targets -- -D warnings
bash scripts/test.sh
```

La app de ajustes se puede abrir fuera de Hyprland para usar otras secciones; Pantallas informa el error y permite volver a detectar. La gestión real de monitores y la revisión visual requieren una sesión gráfica accesible. El proceso interno `--watch-widgets` transmite los ajustes a Eww en JSON y conserva la última configuración válida ante un archivo dañado.
