# Centro de control Eww

Paneles nativos para audio, Wi-Fi, brillo, calendario, Bluetooth, AirPods, energía y apagado.

Solo se muestra un panel a la vez, incluido el reproductor. Abrir otro cierra el anterior; Escape, la «x» y un clic afuera (también sobre la barra o en otro monitor) cierran el panel. Los clics afuera conservan su acción en la aplicación o la barra.

Los controles temporales usan `hyprctl eval` y la API Lua de Hyprland. Cada apertura tiene un identificador para ignorar cierres atrasados de paneles anteriores. Los paneles GTK notifican cuando su ventana está visible; si el inicio falla, el gestor termina el proceso para evitar ventanas huérfanas.

Los servicios están en `desktop/systemd/user/`. Usa `settings.local.json` para seleccionar dispositivos personales; ese archivo no se publica. Las contraseñas de Wi-Fi se solicitan en el diálogo nativo y no se guardan en el repositorio.

Ejecuta las pruebas desde la raíz con `bash scripts/test.sh`. Consulta el README principal para instalación y requisitos.
