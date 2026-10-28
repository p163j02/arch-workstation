# Eww portable

`desktop/sysmon/` contiene el monitor de recursos actual. Los enlaces relativos de la raíz conservan sus puntos de entrada: `eww.yuck`, `eww.scss` y `scripts/`.

`control-center/` contiene los paneles de Waybar, sus adaptadores nativos y sus pruebas. Los servicios están declarados en `shared/systemd/user/` y el arranque y cierre de sesión se gestionan desde `shared/hypr/`.

El instalador compila `scripts/fast-client.c` y configura los dos puntos de entrada Python para utilizarlo. El exportador elimina la ruta de ese intérprete local al guardar el código fuente. No se versionan binarios, cachés ni grafos.

`settings.json` contiene valores portables. La selección de dispositivos propia del usuario va en `settings.local.json`, excluido del repo y preservado por el instalador.

Consulta el README principal para instalación, pruebas, respaldo y selección local de dispositivos.
