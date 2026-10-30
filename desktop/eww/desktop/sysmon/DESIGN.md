# Monitor de recursos — JetBrains Mono / paleta viva de Waybar

## Referencia y alcance
Rediseño visual basado en `~/descargas/monitor-design.png`. El documento `~/descargas/especificaciones-diseno-eww-jetbrains.md` solo aporta tipografía, iconografía y acabados: no determina estructura, orden ni comportamiento.

Se conserva la estructura de archivos, los enlaces `eww.yuck`, `eww.scss` y `scripts`, las ventanas `sysmon-widget` / `sysmon-widget-ext` y el orden **CPU → disco → RAM → temperaturas → Top CPU → Top RAM**. No se modifica Waybar, el centro de control ni el wallpaper.

## Sistema visual
- Panel oscuro opaco, ligeramente teñido por el acento efectivo de Waybar. Padding horizontal de 18 px y separación entre módulos de 10 px.
- Borde, radio y sombras se copian exactamente del contorno de módulos generado por Waybar Editor. Se conserva el listener y su actualización sin reiniciar Eww. El radio observado es 20 px; si Waybar lo cambia, el widget lo sigue.
- JetBrainsMono Nerd Font, ya instalada. Títulos de 18 px / 700, porcentajes de CPU y disco de 28 px / 700, RAM de 24 px, temperaturas de 27 px y procesos de 12 px.
- Encabezados con icono y título a la izquierda, porcentaje a la derecha. Datos secundarios bajo el título, alineados con su texto.
- Un único separador fino entre módulos, sin líneas duplicadas bajo las cabeceras, puntos ornamentales, resplandor en los números ni cajas alrededor de cada proceso.
- Dos tarjetas de temperatura con radio 12 px y pequeños historiales reales conservados, de menor protagonismo.
- Filas de procesos abiertas: chevrón, nombre, consumo y barra de 7 px. Las barras son cajas GTK con longitud calculada sobre una pista de 274 px; no dependen del estilo del progressbar del tema GTK instalado.

## Paleta sincronizada
`waybar-border.py` sigue validando el CSS efectivo de `~/.config/waybar/style.css`: solo lee tras cambios en inode, mtime o tamaño, detecta reemplazos atómicos y conserva el último contorno válido frente a un archivo incompleto o ausente.

Además de `style`, ahora emite tokens derivados mediante `monitor_theme.py`:
- `primary`: RGB exacto del borde, sin su alfa, para iconos, porcentajes y gráficos. El alfa de Waybar se mantiene en el contorno, no en el texto.
- `soft`: acento aclarado para temperaturas normales, valores y barras de procesos.
- `text`, `muted`: blancos y tonos secundarios teñidos por el mismo acento.
- `background`, `card`, `track`, `separator`: mezclas oscuras del acento, manteniendo contraste y la familia visual de la referencia.
- `warning` / `danger`: ámbar y rosa reservados a carga elevada y temperaturas de advertencia/críticas; no se sustituyen por tonos puramente estéticos.

No hay selector Aurora/Nord/Rose ni paleta independiente que pueda desincronizarse. El panel adopta la familia cromática de Waybar manteniendo la composición y acabados de la imagen. No copia la transparencia, fuente o espaciados de Waybar.

Los SVG CPU/disco y las tendencias de temperatura consultan el mismo tema validado y conservan el último válido. Actualizan el color en su siguiente muestreo (CPU/disco 2 s, temperaturas 5 s), sin vaciar el historial. La barra de RAM usa los tokens GTK del listener directamente. El contorno y los elementos GTK responden al listener de 0,5 s.

## Iconos
Glifos reales de la tabla del documento en `JetBrainsMono Nerd Font`: CPU ``, disco ``, RAM ``, temperatura ``, GPU ``, Top CPU ``, Top RAM ``, frecuencia `` y chevrón ``. Los PNG previos permanecen intactos pero no se cargan.

## Datos y comportamiento conservados
- Mismas fuentes de datos y ventanas, ancho 352 px, anclaje superior derecho, desplazamientos 12/25 px, capa inferior, sin reservar espacio ni recibir foco.
- CPU: uso total de `/proc/stat`, 60 muestras reales cada 2 s, escala adaptativa con mínimo 0–10 %, núcleos físicos y frecuencia media actual. SVG de 304 × 52 px.
- RAM: memoria real actualizada cada 3 s, representada por una **barra horizontal de 304 × 18 px y escala 0–100 %**, como en la imagen. No se genera ni muestra un historial de RAM.
- Disco: capacidad de `/` actualizada cada 30 s (porcentaje, GiB y filesystem). **La curva mide E/S real, no ocupación**: suma de bytes/s leídos y escritos por el dispositivo físico que contiene `/`, con 60 muestras cada 2 s. Se resuelve la partición hacia su disco y se desenvuelven dispositivos apilados sin duplicar sus contadores. Los sectores de `/sys/dev/block/*/stat` se convierten a bytes multiplicando por 512 y se dividen por el tiempo monotónico real entre muestras. SVG 304 × 48 px con escala adaptativa y mínimo 1 MiB/s. Lectura y escritura se muestran por separado debajo; el tooltip explica la curva y su escala. Arranque, contador reiniciado y error se muestran como espera/no disponible, no como mediciones fabricadas. En reposo la curva puede ser plana.
- Temperaturas: dos sensores e historiales de 24 muestras cada 5 s. Se conservan los umbrales de 70/85 °C y el comportamiento previo de sensor ausente (0 °C).
- Estados Normal/Elevada/Alta conservados en tooltips; los porcentajes pasan a ámbar/rosa cuando corresponde.
- Cuatro procesos por lista. CPU conserva el valor por núcleo de `ps`; su barra se limita a 100 %, sin limitar el número mostrado. RAM conserva RSS convertido a MiB; barras relativas al primer proceso (máximo de la lista). Ambas escalas están explicadas en tooltips.
- Solo SVG en directorios temporales privados; no hay escrituras periódicas persistentes ni valores ilustrativos del mockup usados como mediciones.

## Verificación de esta revisión
- 25 pruebas offline: CPU (4), RAM/E/S (9), contorno/listener (7) y tema/contraste/sincronización de gráficos (5). Todas pasan. Incluyen unidades de sectores, tiempo real transcurrido, lectura/escritura independientes, arranque y reinicio del contador, partición hacia disco, dispositivos apilados y RAM sin gráfico.
- Pruebas de paleta con colores hex de tres/seis dígitos, RGB y RGBA; respaldo, recuperación, retención frente a escritura incompleta y reemplazo atómico. CSS de prueba únicamente en directorios temporales.
- Render real inspeccionado mediante una instancia overlay temporal: **352 × 988 px**, origen vertical 75 px en la pantalla disponible de 1920 × 1080, escala 1. Todas las filas y el borde inferior caben. La ventana de producción mantiene la capa inferior. No había un segundo monitor conectado para verificar su render; la definición externa permanece intacta.
- Detector mecánico sin hallazgos antes del ajuste final de compatibilidad GTK. El render final confirma fuente, iconos, datos y barras proporcionales sin depender de CSS de navegador.
- Copia del primer rediseño en `~/.local/state/eww-redesign/20261026-012919/`; respaldo previo a la corrección RAM/E/S y captura nueva en `~/.local/state/eww-redesign/20261026-020710-ram-io/`. La instancia temporal de inspección se cierra al terminar.

## Corrección solicitada por el usuario
La instrucción de conservar la estructura se aplicó inicialmente también a las métricas. Tras las aclaraciones, el orden es CPU → disco → RAM, como en la referencia. Se mantienen las ventanas, se elimina el historial de RAM y se sustituye la curva de ocupación del disco por su actividad de lectura/escritura. Esta sección y las descripciones de datos anteriores reflejan el estado corregido, no la primera implementación.

## Ajustes desde Configuración del PC

La app Rust `pc-settings` permite ocultar secciones, ajustar el fondo y elegir anclaje/desplazamiento. Eww recibe los valores mediante `deflisten pc_settings`; los valores iniciales conservan el diseño original. El orden de las secciones visibles y sus recolectores se mantiene. Consulta `desktop/apps/pc-settings/README.md`.
