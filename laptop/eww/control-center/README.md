# Centro de control Eww

Paneles nativos para audio, Wi-Fi, brillo, calendario, Bluetooth, AirPods, energía y apagado.

Los servicios están en `laptop/systemd/user/`. Usa `settings.local.json` para seleccionar dispositivos personales; ese archivo no se publica. Las contraseñas de Wi-Fi se solicitan en el diálogo nativo y no se guardan en el repositorio.

Ejecuta las pruebas desde la raíz con `bash scripts/test.sh`. Consulta el README principal para instalación y requisitos.

Los paneles reciben la paleta del fondo desde `wallpaper-studio.scss`, generado por Wallpaper Studio al aplicar. Incluye superficies, texto, iconos, selección, sliders, calendario y baterías. El gestor recarga únicamente la instancia del centro de control; el panel Wi-Fi nativo detecta las actualizaciones del archivo mientras está abierto. Los avisos de error y las acciones destructivas mantienen su significado visual.

El brillo físico funciona con el backlight de un portátil, un monitor externo DDC/CI o ambos. En un desktop, la ausencia de `/sys/class/backlight` no deshabilita el monitor. Si NVIDIA no ofrece el enlace `ddc` en sysfs se consulta `ddcutil detect --brief`. No se elige un bus arbitrario cuando hay varios monitores: usa `external_connector` y, si no se puede asociar automáticamente, `ddc_bus` en `settings.local.json` (p. ej. `{"external_connector":"DP-3","ddc_bus":"7"}`, usando los valores de `ddcutil detect --brief` de tu equipo).

DDC/CI debe estar habilitado en el menú del monitor y el usuario necesita acceso al dispositivo `/dev/i2c-N`. El panel muestra errores de detección, permisos o lectura y permite **Volver a detectar pantallas**. No usa el atenuador de luz nocturna como sustituto del brillo físico. Las lecturas DDC se conservan brevemente en caché; desconectar o cambiar un monitor invalida esa asociación. Un fallo en una pantalla no impide actualizar la otra.
