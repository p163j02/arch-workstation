# Eww: escritorio y paneles de Waybar

Dos configuraciones independientes; no mezclar sus daemons ni ejecutar `eww kill` para administrar el centro de control.

## Organización

```text
eww/
├── eww.yuck -> desktop/sysmon/eww.yuck
├── eww.scss -> desktop/sysmon/eww.scss
├── scripts -> desktop/sysmon/scripts/
├── desktop/sysmon/          # Monitor de recursos del escritorio
│   ├── eww.yuck            # sysmon-widget y sysmon-widget-ext
│   ├── eww.scss
│   └── scripts/            # CPU, RAM, disco, temperatura y procesos
├── control-center/         # Configuración Eww independiente para Waybar
│   ├── eww.yuck
│   ├── eww.scss
│   ├── settings.json
│   ├── widgets/            # Nueve paneles
│   ├── scripts/            # Adaptadores y gestor de popups
│   ├── tests/              # Tests sin modificar dispositivos
│   ├── maintenance/        # Reversiones históricas; NO ejecutarlas para ordenar
│   └── docs/               # README y resultados históricos
└── graphify-out/            # Análisis de todo el árbol, sin HTML
```

`control-center/README.md`, `TESTS.md`, `scripts/test_adapters.py` y `scripts/rollback*.sh` son enlaces de compatibilidad. El grafo anterior del centro de control se conserva en `control-center/graphify-out/` como análisis histórico, no como cobertura completa del árbol actual.

## Rutas estables

- Escritorio: continuar usando `eww daemon` y `eww open sysmon-widget`; la raíz sigue siendo `~/.config/eww`. No iniciar otro daemon con `-c desktop/sysmon`: sus polls conservan las rutas estables de la raíz.
- Waybar: `~/.config/eww/control-center/scripts/popup-manager.py <panel>`.
- Servicio: `waybar-control-center.service` usa `-c ~/.config/eww/control-center`.
- Pruebas: `python3 ~/.config/eww/control-center/tests/test_adapters.py` (también funciona la ruta anterior).

Se conservaron los nombres de ventanas, las rutas externas, los archivos de interfaz y la resolución `backend.ROOT`. Waybar, Hyprland y los servicios no requieren cambios.

## Flujo del centro de control

Clic Waybar → popup-manager → Eww `cc-*` → `deflisten` → `backend.py watch <módulo>` → señales DBus / `pactl subscribe` / temporizadores → `status()` → adaptador nativo → JSON por stdout → widgets Yuck → estilos SCSS.

Los botones y sliders usan `backend.py action`; los listeners reflejan después el estado real. Calendario y AirPods tienen listeners propios; AirPods además publica JSON para Waybar desde el IPC de airpods-tui.

## Respaldo de la reorganización

La ruta del respaldo completo previo figura en `~/.local/state/eww-organization/latest-backup`. Contiene `eww/` y `checksums.json`, incluyendo hashes de los puntos de entrada externos. No restaurar sobre una configuración en uso sin cerrar primero los paneles y detener sus servicios: la restauración completa eliminaría cambios posteriores.

La documentación de `control-center/docs/` contiene historial y algunas afirmaciones antiguas que su sección de unificación reemplaza. El informe del grafo distingue código actual de documentación histórica.
