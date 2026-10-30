# Waybar Editor

Editor gráfico local en **Rust + GTK4 + libadwaita**, en español. No necesita navegador, servidor ni privilegios de administrador.

## Configuración integrada

El perfil desktop incluye [Configuración del PC](../pc-settings/README.md): abre `pc-settings` para usar Wallpaper Studio y Waybar Editor dentro de una misma ventana, administrar pantallas y ajustar la transparencia y el contenido de los widgets. Los accesos independientes siguen disponibles.

## Abrir

Una vez instalado, busca **Waybar Editor** en Rofi o ejecuta:

```sh
waybar-editor
```

El mismo instalador incluye **Wallpaper Studio**, un gestor de fondos en Rust/GTK4. Busca imágenes en `~/wallpapers` y las carpetas añadidas, con miniaturas, búsqueda por nombre/ruta y vista previa de GIF animados. Ábrelo desde Rofi o ejecuta:

```sh
wallpaper-studio
```

Selecciona un fondo para extraer su paleta. La extracción agrupa los tonos de una muestra de hasta 128 × 128 píxeles, da más peso a las zonas saturadas y elige un secundario separado del acento cuando la imagen contiene otro tono. Ajusta luminosidad conservando tono y saturación, en lugar de mezclar todos los colores oscuros con blanco. Fondo y texto reciben un tinte de la imagen; los fondos monocromáticos conservan colores de su propia familia. Las imágenes parecidas pueden producir paletas parecidas. Puedes editar **Acento**, **Secundario**, **Fondo** y **Texto** con el selector nativo. **Guardar ajustes del fondo** conserva la paleta y la personalización de fastfetch sin cambiar el escritorio; **Extraer de nuevo** recalcula colores de la imagen con el algoritmo actual en el borrador. Las paletas ya guardadas se respetan al actualizar: usa este botón y luego Guarda o Aplica para renovarlas. **Aplicar fondo y paleta** guarda la paleta, el último fondo y la transición, y actualiza todos los monitores. Admite fundido, barrido, onda, círculos, transición aleatoria y cambio instantáneo; la duración es de 0,1 a 10 segundos. Las miniaturas, la extracción de colores y la aplicación mediante `awww` se procesan en segundo plano. La vista previa de GIF se decodifica a tamaño reducido en GTK.

En desktop, **Personalizar fastfetch** guarda por wallpaper el perfil, la imagen, los textos y el ancho. La biblioteca incluye cuatro imágenes originales y 19 variantes Silphwave con miniaturas y filtros por tema. Los colores de datos, etiquetas y marcos se eligen entre los mismos 16 colores ANSI de Kitty, con muestras visibles antes de seleccionarlos y valores predeterminados que siguen la paleta. El instalador copia las 23 imágenes; las preferencias personales se guardan localmente. Consulta [Fastfetch por fondo](../wallpaper-studio/README.md#fastfetch-por-fondo) para usarlo, restaurar el respaldo o sincronizar desde la terminal.

Cada paleta se conserva por ruta absoluta en `$XDG_STATE_HOME/wallpaper-studio/state.json` (o `~/.local/state/...`), con compatibilidad con el estado anterior. Aplica colores a Waybar, el monitor de recursos Eww y los bordes activos/inactivos de Hyprland. Los bordes se guardan en un bloque de `hypr/local.conf`, que la configuración de este repositorio carga al final, para conservarlos tras recargar Hyprland. Eww comparte la paleta entre el widget y sus gráficos, incluso si solo cambia el texto o el fondo. Los desplegables de audio, brillo, Bluetooth, calendario, energía, AirPods y apagado importan `eww/control-center/wallpaper-studio.scss`, generado por el gestor. Al aplicar se recarga esa instancia de Eww; el panel Wi-Fi nativo observa el mismo archivo y actualiza su CSS en vivo.

El CSS original de Waybar y los ajustes de `local.conf` se conservan fuera de bloques marcados. La paleta también se aplica al texto y los iconos de escritorios, al fondo del escritorio activo y a todos los lanzadores centrales, incluidos los añadidos desde el editor. Los lanzadores comparten el secundario en reposo y el acento al pasar el cursor; el icono de Wallpaper Studio sigue la misma regla. La prioridad del tema supera las reglas de colores del editor. Los colores de alarma de batería y apagado se conservan. Se validan colores/estado y se serializan las escrituras con un bloqueo entre procesos; un estado JSON dañado se informa sin sobrescribirlo. Si `awww` falla, se intenta restaurar el CSS de Waybar, los bordes y el tema de los desplegables anteriores. Son reemplazos atómicos por archivo, **no una transacción atómica**: un cierre forzado entre escrituras puede dejar un cambio parcial. Si falla una actualización en vivo tras aplicar el fondo, se muestra la advertencia correspondiente.

`wallpaper-studio --restore` restaura el último fondo al iniciar sesión o cambiar monitores; antes de elegir uno desde Studio conserva la restauración de Waypaper. También acepta `wallpaper-studio --apply /ruta/imagen.gif` y `--help`, sin abrir la interfaz. Necesita `awww-daemon` activo para aplicar. Admite imágenes estáticas y GIF; no incluye reproducción de vídeo ni configuraciones independientes por monitor.

## Uso

- **Barra:** cambia el orden con las flechas o arrastra módulos entre zonas. El menú de cada módulo permite moverlo, ocultarlo o editar sus opciones. **Añadir módulo** abre el catálogo.
- **Desktop y laptop:** el catálogo ofrece batería solo si detecta una batería del sistema y brillo de portátil solo si existe un dispositivo de retroiluminación. Las baterías de periféricos no activan la opción. Los módulos que ya estén configurados, incluidos los de presets importados, siguen disponibles para editar o restaurar. El perfil desktop no incluye módulos de batería ni retroiluminación de portátil.
- **Módulos ocultos:** al principio de Barra aparece una sección desplegable con el número de módulos configurados que no están en ninguna zona ni grupo visible. Se abre automáticamente cuando hay ocultos y al ocultar un módulo. Cada fila muestra su nombre e identificador: elige **Izquierda**, **Centro** o **Derecha** y pulsa **Restaurar**. Conserva opciones, scripts y estilos; solo cambia el borrador hasta confirmar **Aplicar**. La lista se actualiza al cargar presets/respaldos. Los módulos sin opciones propias ocultados con esta versión conservan un objeto vacío para poder recuperarlos después; si uno antiguo ya no aparece en la configuración, puedes añadirlo desde el catálogo.
- **Aplicaciones:** busca entre las aplicaciones instaladas o añade un comando personalizado. Elige nombre, icono y zona. **Explorar iconos** abre un catálogo local de más de 10.000 glifos Nerd Fonts con búsqueda por nombre y filtro por colección; la selección también acepta texto escrito o pegado. Las apps nuevas usan el lanzador `~/.config/hypr/launch-in-workspace.py` previamente instalado para recordar el workspace desde el que se abren. Una app que reutilice una ventana existente puede comportarse de otra manera.
- **Apariencia:** pulsa la muestra de color para abrir el selector nativo; usa la paleta o **+** para un color personalizado y su transparencia. Al confirmar, aparece el código editable; editar el código también actualiza la muestra. Disponible para fondo, texto/iconos, workspace activo y borde. **Bordes y neón** permite grosor de 0 a 6 px (0 lo oculta), activar/desactivar el neón e intensidad de 0 a 100 %. La vista previa cambia al ajustar los controles, sin escribir archivos. **Guardar apariencia en el borrador** conserva esos ajustes en memoria; **Guardar preset** y **Aplicar** también incorporan automáticamente la apariencia pendiente tras validarla. Los controles de posición/monitores siguen requiriendo **Guardar ajustes en el borrador**.
- **Colores de los módulos:** en Apariencia → Colores, **Conservar colores propios de los módulos** respeta las reglas originales (batería según carga, apagado rojo, estados y hover). Desactivado usa el color de **Texto e iconos** para todos. Se guarda por preset. Los presets antiguos mantienen su modo uniforme; activa el interruptor y confirma **Aplicar** para recuperar sus colores originales, que siguen dentro del CSS. La vista GTK3 permite comparar estados de batería de ejemplo, sin consultar ni cambiar la batería real.
- **Vista previa:** usa el CSS del borrador con GTK3, como Waybar, para representar fuentes, iconos, tamaños, espaciado, colores y bordes. Por defecto se reduce al ancho disponible; **Tamaño real** permite recorrerla con scroll sin reducirla. Los botones **75 %**, **25 %**, **10 %** y **Cargando** aparecen solo cuando hay una batería en alguna zona o grupo visible del borrador; solo cambian la batería de ejemplo y respetan los umbrales de la configuración. No guardan cambios ni afectan Waybar.
- **Presets:** guarda el borrador con un nombre; carga, duplica, renombra, elimina, importa o exporta presets. Cargar **no** aplica. Para actualizar un preset, guarda uno con otro nombre; no se sobrescriben archivos existentes.
- **Respaldos:** carga una copia anterior como borrador, revísala y pulsa **Aplicar** para restaurarla.
- **Aplicar:** muestra un resumen, comprueba cambios externos, guarda un respaldo y escribe los archivos. Después solicita a Waybar una recarga (`SIGUSR2`). Si Waybar no está ejecutándose, solo se guardan los archivos.

Atajos: `Ctrl+Enter` para aplicar; `Ctrl+Shift+S` para guardar un preset.

El editor **no cambia la barra activa al abrirse**. Los cambios del borrador se mantienen en memoria; guarda un preset si quieres conservarlos sin aplicar.

## Archivos y seguridad

Por defecto:

- Configuración: `$XDG_CONFIG_HOME/waybar/config.jsonc`, o `config` si no existe JSONC.
- Estilos: `$XDG_CONFIG_HOME/waybar/style.css`.
- Presets: `$XDG_DATA_HOME/waybar-editor/presets/*.json`.
- Respaldos: `$XDG_DATA_HOME/waybar-editor/backups/*.json`.

Sin las variables XDG se usan `~/.config` y `~/.local/share`. Los bundles contienen `version: 1` y el JSONC/CSS original; se guardan con permisos `0600`.

Se preservan comentarios, opciones desconocidas y scripts referenciados. La apariencia añade un bloque delimitado por `/* waybar-editor:begin */` y `/* waybar-editor:end */`, sin regenerar el resto del CSS. Conserva las fuentes de respaldo existentes. Bordes y neón se guardan dentro del CSS de los presets; los presets anteriores siguen siendo compatibles. Los ajustes visuales son comunes a las tres zonas, no estilos independientes por zona.

**Importa solo presets de confianza.** Pueden contener comandos de Waybar: los `exec` pueden ejecutarse al aplicar y recargar. El editor no instala programas, no copia scripts y no ejecuta los comandos de las apps al seleccionarlas. La vista GTK3 no recibe `exec` ni acciones de clic/scroll; sí interpreta CSS y puede leer recursos referenciados por estilos, como imágenes/importaciones. No es un sandbox para temas desconocidos. La advertencia de dependencias revisa rutas explícitas de forma orientativa; no analiza todo el shell ni garantiza que todos los programas estén disponibles.

Cada archivo se reemplaza mediante un temporal sincronizado. Si falla la escritura del CSS se intenta restaurar JSONC. **No es una transacción atómica de dos archivos** y un cierre forzado entre escrituras puede dejar versiones distintas; los respaldos permiten recuperar ambas. Se detectan cambios externos antes de escribir: evita editar simultáneamente con otros programas durante la aplicación. Se rechazan archivos activos que sean enlaces simbólicos o apunten al mismo archivo.

## Alcance de esta versión

- Vista previa **GTK3 con el CSS del borrador**, no una instancia real de Waybar. Usa widgets y formatos comunes con datos de ejemplo: no ejecuta módulos, scripts, clics ni consultas de estado. Textos dinámicos, iconos de ventanas/workspaces y tray no serán idénticos a los reales; módulos/formatos desconocidos pueden usar un ejemplo simplificado. La transparencia se compone sobre el fondo del editor, no sobre tu wallpaper, y no se simulan efectos del compositor. Si GTK3 falta o el CSS no puede dibujarse, se indica el error y aparece una vista esquemática de respaldo.
- Una sola barra con uno o varios monitores; los monitores comparten el mismo diseño. Las configuraciones multibarra con un arreglo JSON raíz se rechazan sin escribir. No hay diseños independientes por monitor en esta versión.
- El guardado valida CSS **estructuralmente** (comentarios/cadenas/llaves) y los valores de los controles visuales. La vista además intenta cargarlo con GTK3 y avisa si falla, pero no se garantiza compatibilidad semántica con todo CSS/widget/recurso arbitrario importado.
- No es un editor visual de todos los módulos ni de todas las reglas CSS; las opciones avanzadas de un módulo admiten JSON y conservan las claves omitidas.
- Los iconos de botones son glifos o texto, no rutas a imágenes. El catálogo incluido procede de [glyphnames.json de Nerd Fonts](https://github.com/ryanoasis/nerd-fonts/blob/master/glyphnames.json) (v3.5.1) y funciona sin conexión; no descarga ni instala fuentes. Para mostrar los glifos en Waybar se necesita una Nerd Font instalada y configurada.
- El lanzador se resuelve desde `$XDG_CONFIG_HOME/hypr/launch-in-workspace.py`, o `~/.config/hypr/launch-in-workspace.py` cuando XDG no está definido. Ambos perfiles de Arch Workstation lo instalan.

## Compilar e instalar

Requiere Rust ≥ 1.89/Cargo, pkgconf, GTK4 ≥ 4.10 y libadwaita ≥ 1.5. La vista fiel requiere además Python3, PyGObject, Cairo y GTK3 con sus typelibs. El generador GTK3 es un helper interno incluido en el binario: devuelve una imagen PNG en memoria y se mantiene separado de GTK4, sin navegador ni servidor. Si falta, el editor conserva el fallback esquemático. El instalador completo de Arch Workstation prepara estas dependencias para laptop y desktop.

```sh
cd ~/arch-workstation
bash desktop/apps/waybar-editor/install.sh
```

Instala `~/.local/bin/waybar-editor`, `~/.local/bin/wallpaper-studio`, `~/.local/bin/pc-settings` y entradas de aplicaciones en `$XDG_DATA_HOME/applications/`. El código fuente queda en `desktop/apps/waybar-editor` dentro del repositorio. Compila con `--locked`; requiere acceso a crates.io si las dependencias no están en caché.

Para validar sin abrir la interfaz:

```sh
waybar-editor --check
waybar-editor --config /ruta/config.jsonc --style /ruta/style.css --data /ruta/datos
```

Pruebas y análisis:

```sh
cargo test --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml
cargo clippy --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml --all-targets -- -D warnings
# Regresión de clic/teclado en miniaturas; requiere una sesión gráfica disponible:
cargo test --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml thumbnail_activation_uses_parent_signal -- --ignored --test-threads=1
```

## Desinstalar

Elimina `~/.local/bin/wallpaper-studio`, `~/.local/bin/waybar-editor` y `$XDG_DATA_HOME/applications/waybar-editor.desktop` (por defecto `~/.local/share/applications/waybar-editor.desktop`). Elimina también `wallpaper-studio.desktop`, `pc-settings.desktop` y el enlace `~/.local/bin/pc-settings`. Los bloques de colores aplicados permanecen en `waybar/style.css` y `hypr/local.conf`; puedes quitar únicamente los bloques delimitados por `wallpaper-studio:begin` y `wallpaper-studio:end` para recuperar los colores anteriores. Presets y respaldos quedan en el directorio de datos hasta que decidas borrarlos.
