# Mejora de latencia: apertura y sliders

## Respaldo

`~/.local/state/eww-performance/20261022-145743/control-center/`. Mediciones baseline.json y after.json junto al respaldo. No se cambiaron Waybar, systemd, Yuck ni SCSS.

## Cambios

- backend carga Gio/GLib bajo demanda. Acciones de audio, brillo y luz nocturna no pagan su import si no utilizan DBus.
- popup-manager reutiliza listado de ventanas y monitores. La asignación GTK se conserva 30 segundos, solo si coincide la sesión Wayland/Hyprland y la topología completa; cambio de geometría invalida caché.
- Señales DBus/Pulse solicitan refresco agrupado a 40 ms, en lugar de esperar el tick de 1 s.
- Gio observa eventos locales de finalización de acciones/mensajes. El estado mostrado se vuelve a leer del adaptador real; no se publican porcentajes inventados.
- Volumen/luz nocturna agrupan escrituras a 40 ms; brillo conserva 120 ms por DDC/CI.
- El slider de volumen lee solo wpctl get-volume, no enumera salidas en cada acción.
- Tras una escritura DDC exitosa se conserva el valor confirmado y bus/máximo en la caché de 15 s. No se fuerza getvcp después de cada escritura; recuperación periódica sigue reconciliando cambios externos.
- Valores pendientes se escriben atómicamente; se drenan nuevos objetivos absolutos llegados durante una escritura, incluyendo temperatura. Los locks se cierran al salir de action.

## Mediciones

Tres muestras de apertura de calendario anterior: 714, 691, 652 ms (mediana 691).
Cuatro posteriores: 630 ms primera, 363, 351, 350 ms; mediana de las tres aperturas con caché 351 ms, ~49% menos. Es duración del comando, incluyendo instalación de bindings, no tiempo a primer frame.

Import backend por proceso: mediana 282 → 105 ms.
Consulta de estado volumen por proceso: mediana 366 → 169 ms.

Integración real GLib/Gio con estado simulado en directorio temporal: notificación local procesada en ~85 ms. No equivale a latencia de hardware ni renderizado Eww.

12 tests pasan, incluidos últimos objetivos concurrentes, caché DDC después de escritura simulada y publicación atómica. Todos sin cambios de dispositivos. Calendario abierto/cerrado para medir; no se aplicaron cambios de brillo/audio/perfil ni automatización física de arrastre.

## Límites

Primera apertura o revalidación de GTK sigue siendo más lenta. DDC/CI tiene latencia física y consultas periódicas que pueden bloquear el listener. Cada acción sigue arrancando Python; si la mejora no basta, evaluar servicio persistente antes de reescribir en Rust/Go. Los incrementos relativos de rueda conservan su comportamiento anterior de coalescencia; no constituyen suma garantizada de todos los eventos.

`~/.config/eww/graphify-out/` describe la arquitectura anterior a esta mejora; pendiente reextracción. Ver este documento y el código actual para intervalos/rutas vigentes.
