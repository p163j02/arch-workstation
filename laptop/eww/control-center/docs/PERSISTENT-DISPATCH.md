# Backend persistente de baja latencia

## Arquitectura

Las rutas existentes backend.py y popup-manager.py usan un cliente nativo pequeño como intérprete shebang. Para acciones/popups, envía JSON por socket Unix al servicio Python persistente; status/watch/cleanup siguen ejecutándose en Python. Si no puede conectar, ejecuta Python como alternativa. Nunca repite una solicitud ya enviada, para no duplicar toggles.

- Servicio: `waybar-control-dispatch.service`, habilitado en sesión del usuario; no requiere root.
- Servidor: `scripts/dispatch-service.py`.
- Cliente: `scripts/fast-client`, compilado desde `scripts/fast-client.c` con GCC.
- Socket: `$XDG_RUNTIME_DIR/waybar-control-center/dispatch.sock`, permisos 0600 y comprobación SO_PEERCRED del UID.
- Cola de acciones generales limitada a 64; mailbox de sliders mantiene solo el último objetivo absoluto por control y acumula pasos relativos. Módulos distintos pueden trabajar en paralelo; sliders del mismo módulo serializados.
- GTK inicializado y bombeado en el hilo principal, no en workers. Se sigue verificando topología por clic y revalidando mapeo GTK cada 30 s; no se mantienen índices inventados.
- Volumen/luz nocturna sin debounce artificial adicional en el servicio; brillo mantiene 120 ms por DDC/CI.
- Respuesta OK para slider confirma aceptación, no éxito físico. Errores posteriores se publican por el mecanismo de mensajes existente y journal.
- No cambian Yuck, SCSS, Waybar ni servicio Eww original. Servicio nuevo utiliza ~22–27 MB en la sesión observada; consumo en reposo pendiente de medición prolongada.

## Medición

Duración de comando de apertura del calendario (no tiempo a primer frame):

- Inicial: mediana 691 ms.
- Optimización anterior: ~351 ms con caché.
- Servicio persistente + llamadas Hyprland agrupadas: mediana 211 ms (5 muestras, 208–217 ms).

Cliente nativo de slider contra servidor sandbox: mediana 2,35 ms (10 muestras). Solo mide arranque/envío/acuse, NO hardware ni renderizado. No se escribió brillo/audio durante esta comprobación.

15 tests pasan (12 adaptadores + 3 dispatch). Compilación GCC con -Wall -Wextra -Werror correcta. Probada alternativa con servicio detenido y cierre explícito: correcta. Servicio reiniciado/activo al terminar. Calendario abierto/cerrado para medición; sysmon conservado. No automatización física de sliders ni pruebas de apagado/conexión.

## Uso y diagnóstico

```sh
systemctl --user status waybar-control-dispatch
journalctl --user -u waybar-control-dispatch -n 50
# Saltarse el servicio para diagnóstico:
CC_DIRECT=1 ~/.config/eww/control-center/scripts/popup-manager.py close
# Recompilar cliente, conservando ruta absoluta del shebang:
gcc -O2 -Wall -Wextra -Werror ~/.config/eww/control-center/scripts/fast-client.c -o ~/.config/eww/control-center/scripts/fast-client
```

Scripts importados por el proceso persistente requieren `systemctl --user restart waybar-control-dispatch` después de editarlos. El shebang absoluto está ligado a esta instalación; si se traslada a otro usuario, hay que ajustar ambos shebangs o invocar explícitamente python3.

## Reversión

Respaldo previo: `~/.local/state/eww-performance/persistent-20261022-150610/control-center/`.

`maintenance/rollback-performance.sh` deshabilita el servicio nuevo y restaura solo backend.py/popup-manager.py, archivando cambios actuales. Comprobado con bash -n, NO ejecutado. No restaura todo el árbol ni altera Waybar o dispositivos; fuentes nuevas quedan sin utilizar.

## Límites

Apertura todavía consulta Eww y geometría; listeners se inician al abrir. DDC/CI tiene latencia física. Procesos watch/status continúan siendo independientes. No se promete latencia cero. El grafo Graphify sigue marcado pendiente de actualización; no generar HTML al actualizarlo.
