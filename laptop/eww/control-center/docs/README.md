# Control Center nativo para Waybar / Hyprland

Implementado el 22-10-2026. Configuración Eww independiente: no se ha modificado `~/.config/eww/eww.yuck` ni `eww.scss`, ni sus dos ventanas sysmon.

## Unificación (estado actual)

- Clic en **batería** abre batería/perfiles: porcentaje pill, estado, minutos UPower y selección real de modo. Quitados los iconos separados de perfil y ampolleta. Waybar power-profiles-daemon usa su propio cambio al clic en esta versión; se dejó de usar como lanzador.
- Autonomía: TimeToEmpty de UPower durante descarga; no predicciones contrafactuales por perfil. Tras elegir modo, la estimación se ajusta al consumo medido. En corriente se muestra ese estado, no minutos inventados.
- Clic en **brillo** abre brillo sincronizado notebook/externo + luz nocturna 2500–6500 K, botón default 3500 K y activación independiente. Menos K = más cálida. Ya no se expone slider de atenuación gamma.
- **Escape, × y clic exterior** cierran. Bindings runtime solo mientras haya popup; clic de ratón no consumidor (pasa a la aplicación). Dentro del popup no cierra. Incluye Waybar y otros monitores; un identificador por apertura evita que un clic atrasado cierre el panel nuevo. Se eliminan solo los bindings Lua propios, sin borrar atajos personales. ExecStopPost retira bindings al detener/fallar el daemon; tras un reload de Hyprland con popup abierto, reabrirlo restablece bindings.
- Icono Wi-Fi duplicado: nm-applet fue iniciado por nuestras pruebas. Se detuvo solo cc-wifi-agent, no NetworkManager. El agente se inicia bajo demanda para conectar y se detiene al terminar; puede aparecer temporalmente al solicitar contraseña. Sin autostart nuevo, no debería persistir tras reiniciar. Su ajuste show-applet fue probado y restaurado a true porque no oculta el indicador en esta compilación.

### Brillo externo: habilitado

**Actualización:** ddcutil 3.0.2-1 instalado por el usuario; permiso I²C confirmado. Consulta y escritura de brillo externo verificadas (27→28→27), y callback sincronizado notebook/externo a 50%, con restauración de originales. Los paquetes incluyen carga de i2c-dev para futuros arranques. La descripción siguiente documenta el bloqueo anterior, ya resuelto.

El bus real de HDMI-A-1 es **i2c-4**. No existen nodos /dev/i2c-* y falta ddcutil; sudo solicita contraseña. El panel lo informa y por ahora ajusta solo notebook. DDC/CI se detecta y consulta por conector/bus real, nunca por índice de pantalla inventado, caché 15 s. Para habilitarlo manualmente:

```sh
sudo pacman -S --needed ddcutil
sudo modprobe i2c-dev
sudo udevadm trigger --subsystem-match=i2c-dev
ls -l /dev/i2c-4
ddcutil --bus 4 --brief getvcp 10
```

Activa DDC/CI en el menú del monitor. Si falta permiso, revisa las reglas udev que instala ddcutil y el acceso requerido por ellas; no usar chmod 666 ni ejecutar el backend como root. Para cargar i2c-dev en futuros arranques, crear conscientemente `/etc/modules-load.d/i2c-dev.conf` con `i2c-dev`. **No se modificó el sistema automáticamente.** Si DDC responde y hay permiso, el mismo slider escribe ambos brillos físicos. El brillo externo aún no está verificado; sin habilitar esto no puede prometerse que funcione. No se usa gamma como sustituto.

Backup de esta unificación: `backup-unified-20261022-062519`. Reversión específica: `scripts/rollback-unified.sh` (no ejecutada). Las secciones siguientes conservan el historial; esta sección prevalece sobre comportamientos anteriores.

## Uso

Clic izquierdo en volumen, red, brillo, reloj, Bluetooth, AirPods, perfil de energía, ampolleta o apagado. Solo hay un popup abierto a la vez. Segundo clic, × o **Super+Escape** cierran. Clic derecho en volumen: mute. Rueda: volumen ±5%, brillo ±5% (mínimo configurable, inicialmente 5%).

Paneles Tokyo Night, JetBrainsMono Nerd Font, borde azul, fondo semitransparente y radio 15 px. No navegador, HTML ni controles simulados.

### Qué está conectado

- **Audio:** volumen/mute de salida predeterminada con wpctl; salidas reales de `pactl -f json list sinks`, seleccionadas por `object.id` validado. Tope 100%. Cambiar salida predeterminada no garantiza migrar streams existentes si una aplicación fija su dispositivo.
- **Wi-Fi:** NetworkManager mediante nmcli. Estado, señal, SSID, conexión/desconexión, radio, búsqueda bajo demanda. Se deduplican AP por SSID/seguridad; se conserva AP activo o más fuerte. Perfiles existentes por UUID; redes nuevas por BSSID, no nombres interpolados en shell. Contraseña solicitada por **agente NetworkManager nativo nm-applet**, no por Eww. Redes ocultas/perfiles avanzados: botón `nm-connection-editor`.
- **Brillo:** sysfs para lectura, brightnessctl para escritura, dispositivo `amdgpu_bl1`. El popup/rueda sobre HDMI no modifica eDP-1. No hay DDC/CI ficticio. Controles deshabilitados y explicación en monitor externo. La barra existente sigue mostrando el porcentaje del backlight del portátil en ambos monitores; se preserva ese módulo.
- **Calendario:** Python estándar, español independiente del locale instalado, lunes primero, hoy, mes anterior/siguiente y fecha seleccionada. No inventa eventos ni los sincroniza.
- **Bluetooth:** BlueZ DBus con Gio: radio, lista conocida/cercana, conectar, desconectar, emparejar. Agente blueman-applet para autorización/PIN. Escaneo de **20 segundos máximo**, cancelado al cerrar/cambiar popup. La conexión DBus del escáner permanece abierta para que BlueZ mantenga la sesión de discovery.
- **AirPods:** `annoyedmilk/airpods-tui` **sin modificar**, compilado `--release --locked`, v0.3.3, commit `8af4eb20cbc39c33983d70c0518753db13f0bbfa`. Servicio headless `--daemon`, IPC oficial `airpods-tui.sock` (u32 big-endian + JSON AppEvent). No se parsea la interfaz TUI. JSON Waybar proviene de un adaptador de ese IPC para poder distinguir datos antiguos. Conexión real BlueZ; salidas reales PipeWire.

### Portales cautivos Wi-Fi: apertura automática

El servicio de usuario `waybar-wifi-portal.service` ejecuta `scripts/wifi-portal.py` independientemente de las ventanas Eww. Escucha eventos DBus de NetworkManager y lee su estado cada 30 s como recuperación; no reescanea redes ni fuerza comprobaciones HTTP adicionales.

Cuando la conexión Wi-Fi activa es la ruta principal/predeterminada y NetworkManager detecta **portal cautivo**, solicita al navegador predeterminado abrir `http://neverssl.com/`. HTTP permite que la red redirija al formulario. Solo lo solicita una vez por conexión, incluso al reiniciar el servicio; al desconectar/reconectar puede volver a abrirlo. No abre por estados «sin Internet», «limitado» ni si hay conectividad completa. Ethernet, Wi-Fi secundario y rutas principales VPN quedan excluidos para evitar abrir el portal por la ruta equivocada.

No guarda ni introduce credenciales ni ejecuta URLs tomadas de SSIDs. Comprueba que el formulario redirigido pertenece a tu universidad antes de iniciar sesión. Depende de que NetworkManager tenga comprobación de conectividad habilitada y de que la red sea detectada correctamente; no todos los portales se detectan. Si una sesión del portal vence sin desconectar, abre `http://neverssl.com/` manualmente para evitar pestañas repetidas. Si `xdg-open` falla después de iniciarse, se registra un aviso y no se insiste durante esa conexión.

```sh
systemctl --user status waybar-wifi-portal.service
journalctl --user -u waybar-wifi-portal.service -n 30
nmcli networking connectivity
python3 -m unittest discover -s ~/.config/eww/control-center/tests -p 'test_wifi_portal.py'
```

Desactivar únicamente esta función:

```sh
systemctl --user disable --now waybar-wifi-portal.service
```

Reactivar: `systemctl --user enable --now waybar-wifi-portal.service`. No requiere sudo ni cambios en NetworkManager, Waybar o los widgets.

### Ampliación: perfil, atenuador y apagado

- **Perfil de energía:** perfiles disponibles y activo de power-profiles-daemon vía DBus; cambios con powerprofilesctl, sin opciones inventadas.
- **Ampolleta:** atenuación real por gamma hyprsunset, 0–80% (gamma 100–20), aplicada a todas las pantallas. No ahorra energía del backlight. Luz nocturna independiente: 3500 K / 6500 K. Reutiliza el proceso existente; si no existe, inicia el servicio de usuario hyprsunset bajo systemd, sin escaneo ni procesos sueltos. Los ajustes son de sesión, no se escribe una programación persistente.
- **Apagado:** las cuatro acciones de wlogout en un popup: hyprlock, systemctl poweroff, hyprctl dispatch exit y systemctl reboot. Bloqueo inmediato; las otras requieren confirmación de 20 s y se cancelan al cerrar. wlogout y su configuración se conservan sin cambios.
- **Wi-Fi:** indicador animado mientras trabaja. Tras solicitar rescan se espera el cambio real de LastScan de NetworkManager (máximo 20 s), no solo la salida de nmcli.
- **Audio/calendario:** altura natural, sin scroll exterior; audio tampoco limita la lista de salidas. Comprobados completos en 1920×1080; alturas extremas con muchas salidas o pantallas muy pequeñas no verificadas.

## AirPods: limitaciones verificadas

Dirección configurada: `20:78:CD:23:16:0A`, “AirPods de Felipe”. Modelo identificado automáticamente por product_id IPC o Modalias BlueZ; se eliminó la generación declarada fija y su aviso. **BlueZ devuelve `bluetooth:v004Cp200FdD415`**, producto **0x200F (2ª generación)**. Productos desconocidos muestran «Modelo no identificado», sin adivinar. El panel conserva únicamente funciones comunes: conexión, batería y salida. **No ANC, transparencia, adaptativo ni conversación.**

Durante las pruebas pasó de desconectado a conectado y PipeWire creó la salida “AirPods de Felipe”. IPC/upstream entregó **99% izquierdo / 99% derecho**, sin batería de estuche ni control de detección de oído. No se atribuye esa conexión a un clic automatizado: las pruebas no ejecutaron conectar/desconectar ni reemparejar el dispositivo.

- Batería sin datos: `—`. No se calculan baterías inexistentes.
- Snapshot IPC no contiene fecha de medición: se muestra **`N% · anterior`**, no se considera actual. Datos en vivo de auriculares expiran en 180 s (configurable). El estuche se conserva siempre como anterior: upstream puede retenerlo, no permite verificar su frescura.
- Waybar muestra el mínimo izquierdo/derecho **solo si está fresco**, o Battery1 BlueZ si disponible. Un `—` con AirPods conectados es válido. Cada instancia de la barra mantiene su propio cliente IPC; no debe llamarse fresco un snapshot sin timestamp.
- Detección de oído aparece **No disponible** salvo respuesta real `EarDetectionConfig` de la sesión. No hay interruptor: no se ha verificado escritura fiable en primera generación.
- Se desactivaron `ble_scan` y `auto_connect` en `~/.config/airpods-tui/config.toml` para evitar búsqueda permanente/robar conexiones. La conexión inicial AACP puede intentar sesión sobre dispositivos ya conectados.
- Upstream advierte que falta Apple DeviceID. **A pesar del aviso se observaron datos de batería**; por tanto no se asegura que falten siempre, ni que AACP sea completo.

### DeviceID opcional (requiere consentimiento y sudo)

No se tocó `/etc/bluetooth/main.conf`. Si quieres completar el handshake recomendado por upstream, lee primero `~/.local/share/waybar-control-center/vendor/airpods-tui/README.md`, sección Apple DeviceID setup. Respalda antes:

```sh
sudo cp -a /etc/bluetooth/main.conf /etc/bluetooth/main.conf.before-airpods
sudoedit /etc/bluetooth/main.conf
```

Añade/ajusta **una sola** línea bajo `[General]`:

```ini
DeviceID = bluetooth:004C:0000:0000
```

`sudo systemctl restart bluetooth` interrumpe todos los dispositivos Bluetooth. Upstream recomienda olvidar y reemparejar los AirPods que se emparejaron antes del cambio; hazlo manualmente solo cuando puedas asumir esa interrupción. No se ha hecho automáticamente. Puedes emparejar con el panel y el agente Blueman. Reversión del DeviceID: restaura el backup y reinicia Bluetooth.

## Colocación / teclado

Waybar no expone la coordenada de cada módulo a su comando on-click. Se toma **cursor al momento del clic**, se determina monitor Hyprland y se centra/clampa el panel dentro de él con 12 px de margen. Se lee la capa Waybar (altura real 46 px en esta sesión) y `reserved` de Hyprland; el panel empieza 8 px debajo. GTK usa nombres/modelos y orden distinto a conectores Hyprland: se resuelve índice GTK por geometría, no se asume HDMI=0.

En Hyprland 0.57, todos los desplegables (incluido multimedia) se cierran con Escape, su X o al pulsar cualquier botón del ratón fuera del panel, también sobre Waybar u otro monitor. Los clics externos continúan llegando a la aplicación situada debajo; los controles internos siguen funcionando. Solo mientras hay un panel abierto se registran bindings Lua propios, que se retiran por su identificador sin borrar los atajos personales. Super+Escape conserva el cierre explícito. Multimedia y Wi-Fi usan ventanas GTK layer-shell con el mismo gestor y geometría que Eww. Alturas acotadas y scroll en paneles con listas, excepto volumen/calendario, que usan altura natural para no recortarse.

## Archivos

- `~/.config/eww/control-center/eww.yuck`, `eww.scss`
- `widgets/{volume,wifi,brightness,calendar,bluetooth,airpods,profiles,nightlight,power}.yuck`
- `scripts/backend.py`, `airpods.py`, `calendar_state.py`, `popup-manager.py`, `test_adapters.py`
- `settings.json`: dispositivo/monitor brillo, mínimo, selección Wi-Fi/adaptador BT, dirección AirPods, caducidad, ocultar desconectados y anchos.
- `~/.config/systemd/user/{waybar-control-center,airpods-control-center}.service`
- `~/.local/bin/airpods-tui`
- `~/.config/airpods-tui/config.toml`
- Modificados mínimamente: Waybar `config.jsonc`, CSS solo selector AirPods agregado, Hyprland solo inicio de servicios y cierre Super+Escape.
- Repos y documentación auditados: `~/.local/share/waybar-control-center/vendor/`.

Si hay varios adaptadores, el backend exige configurar `wifi_interface` / `bluetooth_adapter` en vez de elegir uno arbitrario. El límite mínimo de brillo se aplica en backend; si lo cambias ajusta también `:min` del scale en `widgets/brightness.yuck`.

## Dependencias / instalación

Todas las herramientas necesarias para esta máquina se encontraron instaladas; airpods-tui fue lo único compilado e instalado localmente. Referencia Arch:

```sh
sudo pacman -S --needed eww waybar hyprland python python-gobject gtk3 \
  networkmanager network-manager-applet blueman bluez bluez-utils \
  pipewire pipewire-pulse wireplumber brightnessctl ttf-jetbrains-mono-nerd \
  git rust dbus pkgconf power-profiles-daemon hyprsunset hyprlock
```

Eww instalado declara `0.5.0 d87c2fd`; el checkout correspondiente se describe como release 0.6.0. Se verificaron propiedades contra su fuente, no se asumió `focusable=ondemand`. Hyprland 0.56.2. Gio/GTK3 son necesarios para DBus e identificación de monitores. NetworkManager, BlueZ y PipeWire deben funcionar; no se cambió el gestor de redes/audio ni se reiniciaron servicios del sistema.

Servicios de usuario habilitados y añadidos al inicio Hyprland; reciben WAYLAND_DISPLAY a través de la línea dbus-update-activation-environment ya existente. Para recompilar:

```sh
cd ~/.local/share/waybar-control-center/vendor/airpods-tui
cargo build --release --locked
install -m755 target/release/airpods-tui ~/.local/bin/airpods-tui
systemctl --user restart airpods-control-center
```

Respalda binario/configuración antes de actualizar. No usar `install.sh` upstream automáticamente: tiene hooks de sistema DeviceID.

## Seguridad / consumo

Nombres externos se usan como datos, nunca como comandos. Solo IDs numéricos, MAC validadas y fechas ISO generadas cruzan los comandos Yuck. Python usa argv/DBus, no shell=True. Contraseñas/PIN se delegan a agentes del sistema; no se guardan en scripts, variables Eww, argumentos ni logs. Los perfiles existentes de NetworkManager siguen su gestión habitual de secretos.

Listeners Eww existen solo mientras se usa su variable en un popup. Audio `pactl subscribe`; Wi-Fi/BT señales DBus, recuperación cada 15 s; brillo sysfs 1 s; calendario local 1 s; AirPods IPC con refresco BlueZ 3 s por cliente. Sin reescaneo Wi-Fi automático. Acciones con timeouts; sliders coalescen, están limitados a porcentajes válidos. Servicios systemd poseen los procesos y paran grupos completos. Agentes nm-applet/Blueman, arrancados solo cuando necesarios, son servicios de sesión persistentes gestionados por systemd, no procesos huérfanos.

## Pruebas / diagnóstico

Ver `~/.local/state/waybar-control-center/tests/` y `TESTS.md` para resultados y capturas reales. Ejecutar tests no modifica dispositivos:

```sh
python3 ~/.config/eww/control-center/scripts/test_adapters.py
systemctl --user status waybar-control-center airpods-control-center
journalctl --user -u waybar-control-center -u airpods-control-center -n 100
~/.config/eww/control-center/scripts/backend.py status wifi
~/.config/eww/control-center/scripts/backend.py status bluetooth
~/.config/eww/control-center/scripts/airpods.py
~/.config/eww/control-center/scripts/popup-manager.py volume
eww -c ~/.config/eww/control-center active-windows
hyprctl -j layers
hyprctl configerrors
```

Recargar Eww con ventana cerrada evita un timeout corto del IPC instalado:

```sh
~/.config/eww/control-center/scripts/popup-manager.py close
systemctl --user restart waybar-control-center
pkill -SIGUSR2 -x waybar
```

No `eww kill` sin `-c`: eso afectaría sysmon. Se observa aviso de tema GTK global `gtk-dark.css: border-spacing` inválido; no procede del SCSS nuevo ni se modificó ese tema ajeno.

## Reversión

Para deshacer **solo esta ampliación** y volver a los seis paneles iniciales:

```sh
~/.config/eww/control-center/scripts/rollback-refinement.sh
```

Respaldo: `~/.local/state/waybar-control-center/backup-refinement-20261022-060620/`, registrado en `refinement-backup`. El script archiva el estado actual, restaura control-center y Waybar, sin tocar sysmon, Hyprland, wlogout ni ajustes gamma/perfil actuales. No ejecutado durante las pruebas. El script siguiente revierte la instalación inicial completa.

Backup completo previo en la ruta registrada por `~/.local/state/waybar-control-center/latest-backup`. El script `rollback.sh` restaura **solo los tres archivos modificados** desde ese backup, deshabilita nuestros servicios y deja los demás archivos/ajustes de escritorio intactos. Antes de revertir hace otro backup de esos tres archivos. Configuración nueva y repos quedan archivados; se pueden borrar manualmente después de verificar.

```sh
~/.config/eww/control-center/scripts/rollback.sh
```

No restaura archivos de sistema porque ninguno fue cambiado. No elimina dispositivos emparejados. No elimina la configuración/binary AirPods nueva para evitar borrar ajustes posteriores inadvertidamente.
