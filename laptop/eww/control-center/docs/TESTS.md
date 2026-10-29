# Pruebas reales y límites de cobertura

## DDC/CI habilitado y verificado

- Usuario completó instalación ddcutil 3.0.2-1. Acceso real a /dev/i2c-4; getvcp 10 devuelve 27/100.
- Escritura externa real 27→28→27 comprobada.
- Callback de slider sincroniza notebook y monitor a 50%; valores originales restaurados después (backlight exacto y externo 27%). Corregido caso donde notebook ya estaba en el porcentaje elegido pero externo no: ahora también se compara el externo antes de omitir escritura.
- Paquetes instalaron /usr/lib/modules-load.d/ddcutil.conf con i2c-dev; no se agregó configuración de sistema propia. Reinicio aún no probado.

## Unificación posterior

- Backup `backup-unified-20261022-062519`, incluyendo unidad de usuario antes de editarla.
- Escape verificado mediante teclado virtual uinput en 8 popups; clic exterior virtual cierra; binding ratón non_consuming confirmado en Hyprland, sin overlay. Bindings temporales desaparecen al cerrar.
- Clic real inyectado en perfil ahorro: cambia powerprofilesctl y no cierra panel. Perfil original equilibrado restaurado.
- × comprobada con clic virtual: se detectó timeout de 200 ms insuficiente y se aumentó a 5 s.
- Temperatura real 3500 → 4000 → 3500, restaurada. Paneles batería/pantallas comprobados visualmente; capturas `tests-unified/`.
- UPower inicialmente corriente/100%; durante pruebas reportó descarga/99%, ≈401 min, reflejados en captura. No se desconectó corriente mediante scripts, no se atribuye cambio al agente.
- Tray RegisteredStatusNotifierItems vacío tras detener únicamente cc-wifi-agent; red conservada. No verificada nueva contraseña después del cambio de lifecycle.
- Brillo externo **no funcional todavía** por falta de ddcutil y /dev/i2c-*; bus sysfs real i2c-4. Se implementó adaptador DDC/CI con caché/timeouts, pero no se ejecutó ninguna escritura externa. Falta instalación/autorización de administrador y prueba con hardware. Brillo de notebook disponible.
- No verificados: predicción independiente para cada perfil (no se inventa), persistencia/arranque nuevo, DDC/CI, rollback-unified. Siete tests originales siguen pasando.

## Ampliación del 22-10-2026

- Respaldo completo antes de cambios: `backup-refinement-20261022-060620` (Waybar/Eww/Hyprland/wlogout y unidades de usuario).
- Volumen, calendario, Wi-Fi, AirPods y tres paneles nuevos abiertos secuencialmente en ambos monitores: una sola ventana, y=54, dentro de 1920×1080. Capturas y geometrías: `~/.local/state/waybar-control-center/tests-refinement/`. Volumen ahora 380×257 y calendario 380×455: contenido completo, sin scroll. Perfil 380×350, atenuador 380×350, apagado 380×440.
- Perfil real equilibrado → ahorro; restaurado al valor original equilibrado.
- Gamma real 100 → 90 (atenuación 10%); luz cálida alternada; gamma 100 y temperatura original 3500 K restauradas.
- Wi-Fi rescan real completado sin desconectar la red; se esperó LastScan. Listener registró cuatro fotogramas del spinner; captura mientras buscaba y log `wifi-watch.jsonl`.
- Apagado: petición Apagar → confirmación visible → cancelar. **No** ejecutados bloqueo, apagado, reinicio ni salida de sesión.
- Modelo automático comprobado con Modalias real 0x200F, más test unitario. Ya no se muestra generación declarada.
- Siete unit tests pasan; py_compile y bash -n pasan. Recarga Eww aislada y Waybar; originales sysmon idénticos al respaldo.
- No verificados: clic físico/arrastre end-to-end, arranque de hyprsunset cuando no existe (se reutilizó el actual), persistencia/nuevo login, pantallas pequeñas o una lista enorme de salidas, rollback-refinement. Scripts y rutas implementados, sin afirmar ejecución.

## Verificado en la instalación inicial

- Compilación upstream airpods-tui `cargo build --release --locked`: completada, release v0.3.3, sin parchear.
- Cinco unit tests: escape nmcli (`:` y `\`), límites de porcentaje/NaN/inyección, batería desconocida, snapshot no fresco, febrero bisiesto/lunes primero. Todos pasan. Los eventos artificiales existen solo en el test, no en paneles/producción.
- Apertura secuencial de los seis popups en **HDMI-A-1 y eDP-1**; una ventana a la vez. Y=54 con barra real de 46 px, gap 8 px. Volumen 380×330, Wi-Fi 380×560, brillo 380×320, calendario 380×430, Bluetooth 380×480. AirPods inicialmente crecía a 538 px por mínimo GTK del progressbar: corregido en SCSS a **420×710**, comprobado al borde derecho de HDMI-A-1 (x=1488, final x=1908 <1920).
- Toggle del calendario devuelve lista de ventanas vacía; cierre explícito cancela listeners. Tras cerrar no quedaron `backend.py watch`, `calendar_state.py watch` ni `pactl subscribe`.
- Configuración Hyprland recargada; `hyprctl configerrors` vacío. Widgets sysmon originales conservados en ambas pantallas.
- Waybar recargada con SIGUSR2; `custom/airpods` arrancó un cliente por barra, se observa el indicador real. Módulos ajenos se preservaron.
- Volumen real 65→64, mute alternado y restaurado; restauración final verificada. Cambio de salida AirPods→analógico→AirPods verificado con pactl; salida original restaurada. No se inició audio de prueba.
- Brillo real 52→51, restaurado **valor sysfs exacto**. Acción sobre HDMI denegada sin alterar brillo. `actions.json` contiene resultados.
- NetworkManager: SSID/red activa, señal real, BSSID validado, estado Conectado. No se desconectó la red ni se cambió radio.
- BlueZ: dispositivo AirPods real y modalias leído. Escaneo StartDiscovery/StopDiscovery: **activo=true**, cancelación por archivo al cierre y **detenido=true, exit=0**. Se detectó y corrigió pérdida de sesión DBus por vida de conexión demasiado corta.
- AirPods pasó a conectado durante la sesión; PipeWire creó salida con nombre real. Upstream `--waybar` y nuestro IPC recibieron izquierdo/derecho 99%; estuche `—`, detección oído no disponible. El snapshot se muestra anterior. No se hizo reemparejamiento ni cambio de `/etc/bluetooth/main.conf`.
- Capturas PNG de paneles reales en `~/.local/state/waybar-control-center/tests/`, inspeccionadas con herramienta de imagen. No son mockups.

## No probado / no afirmar

- Click físico sobre cada control/arrastre de slider: se probaron aperturas, geometría, callbacks/backend equivalentes y estados reales, no automatización de mouse de extremo a extremo.
- Contraseña de red nueva, contraseña incorrecta, red oculta, hotspot, perfil empresarial; requieren otra red y autorización interactiva. Ruta implementada mediante agente NetworkManager, no promesa de prueba completada.
- Emparejar PIN ni conectar/desconectar un dispositivo Bluetooth durante pruebas automatizadas. Agente y llamadas DBus están implementados.
- ANC/transparencia/adaptativo/conversación: deliberadamente no existen.
- Detección de oído de AirPods 1ª generación o escritura de ese ajuste: no verificada, no se expone interruptor.
- Estuche actualizado, batería en vivo de primera generación, reinstalación DeviceID o reemparejamiento: no realizados.
- Escala fraccional, rotación, hotplug mientras popup abierto, sesión nueva tras reboot: cálculo contempla escala/transformación pero solo se probó esta sesión con dos monitores 1920×1080 escala 1. Si se retira monitor con popup abierto, ciérralo y vuelve a abrir.
- Reversión no ejecutada: script comprobado con `bash -n`, backup previo comprobado, no se revierte una instalación activa durante entrega.

## Incidencias corregidas durante desarrollo

Paréntesis Yuck, expresiones `for` JSON, función min no disponible, conflicto `calendar.py` con biblioteca estándar, índice GTK distinto a conector Hyprland, doble desplazamiento por zona exclusiva Waybar y mínimo de progressbar GTK. La versión Eww instalada puede devolver timeout corto de IPC durante reload con ventanas abiertas; se recomienda cerrar y reiniciar solo el servicio aislado. Aviso de tema GTK global border-spacing sigue presente, ajeno a esta implementación.
