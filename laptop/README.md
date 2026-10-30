# Notebook: entorno autónomo sobre Arch Linux

Esta carpeta contiene la instalación, las apps y las configuraciones del notebook capturadas el **29 de octubre de 2026**. Puede copiarse por sí sola: sus instaladores y validaciones utilizan únicamente archivos dentro de `laptop/`. La copia del escritorio se mantiene en `desktop/`.

## Instalar

Parte de Arch Linux x86_64 ya instalado, con un usuario normal que pueda usar `sudo`, arranque y conexión a Internet disponibles. Habilita `multilib` en `/etc/pacman.conf`. Ejecuta desde una terminal como tu usuario:

```bash
cd ~/arch-workstation/laptop
bash install.sh
```

El instalador solicita confirmación, actualiza paquetes, detecta CPU/GPU y headers del kernel, instala los paquetes oficiales y de AUR, recupera las apps Craft verificadas, compila las apps Rust del notebook y aplica las configuraciones. También prepara SDDM/Pixie, Hyprlock, los servicios, GTK y los grupos necesarios. Revisa las recetas AUR cuando se solicite. Necesita Internet y `sudo` para los pasos del sistema.

Al terminar, inicia una sesión nueva de Hyprland cuando decidas cargar el entorno. La configuración nativa `hypr/hyprland.lua` está preparada para Hyprland 0.57. El archivo `hyprland.conf` conserva la referencia del notebook que todavía tenía instalada la configuración anterior.

Si las aplicaciones ya están instaladas y solo quieres aplicar configuraciones:

```bash
bash install.sh --config-only
```

Este modo respalda los archivos sustituidos y compila el cliente IPC de Eww. Los ajustes seleccionados de editores se mezclan con el perfil existente; no se elimina su almacenamiento de sesiones. La selección inicial de Wallpaper Studio se copia solamente cuando no existe un estado personal anterior. Para activar SDDM en una instalación existente, recompila la app y ejecuta una vez:

```bash
bash apps/wallpaper-studio/install.sh
bash apps/wallpaper-studio/setup-login-theme.sh
```

Desde la raíz del repositorio siguen funcionando `bash install.sh laptop` y `bash update.sh laptop`: delegan a las entradas de esta carpeta.

## Contenido de la captura

| Ruta | Contenido |
| --- | --- |
| `install.sh`, `update.sh`, `scripts/` | Instalación, exportación, detección de hardware, normalización y comprobación propias del notebook. |
| `packages/official.txt` | 158 paquetes oficiales: selección instalada explícitamente y herramientas necesarias para compilar y restaurar el entorno. |
| `packages/aur.txt` | 16 paquetes AUR instalados explícitamente. `yay` se prepara en el instalador cuando falta. |
| `packages/craft-releases.json` | Cuatro bundles Flatpak y el AppImage de ArtCraft, con versiones y SHA-256 del proveedor. |
| `packages/flatpak.tsv` | Inventario de las apps Flatpak y sus orígenes locales. |
| `apps/` | Código de Waybar Editor y Wallpaper Studio, integración de SDDM/Hyprlock e instalador verificado de Craft. |
| `hypr/`, `waybar/`, `eww/`, `systemd/` | Ventanas, teclado, monitores, barra, centro de control, monitor de recursos y servicios del notebook. |
| `sddm/`, `system/` | Tema de acceso, zram, swappiness, apariencia GTK y servicios habilitados. |
| `preferences/` | Preferencias seleccionadas de Code, Antigravity, Antigravity IDE, Thunar, htop, LazyDocker, OBS, AirPods y otras herramientas. |
| `local/` | Lanzadores personales normalizados; el AppImage se recupera desde el release oficial. |
| `wallpapers/`, `state/wallpaper-studio/` | Fondos y configuración inicial de colores del notebook con rutas portables. |
| `inventory/` | Fecha y resumen de captura, versiones instaladas y archivos omitidos. Las versiones son evidencia; los paquetes de Arch se resuelven desde sus repositorios al instalar. |

Las apps oficiales/AUR incluyen el entorno gráfico, navegadores, herramientas de desarrollo, Office, TeX Live, Docker, VirtualBox, Steam y utilidades presentes en el notebook. La suite Craft recupera las versiones registradas de [ArtCraft Launcher](https://github.com/storytold/craft-launcher/releases), [GridCraft](https://github.com/storytold/gridcraft/releases), [PdfCraft](https://github.com/storytold/pdfcraft/releases) y [WordCraft](https://github.com/storytold/wordcraft/releases). Una descarga con un hash diferente detiene la instalación antes de sustituir la aplicación.

El paquete instalado `claude-desktop-bin` [fue retirado de AUR](https://github.com/patrickjaja/claude-desktop-extra/issues/207). La receta usa el paquete disponible `claude-desktop`; la versión original queda registrada en `inventory/installed-versions.txt`. Al reinstalar sobre un equipo que conserva el paquete anterior, revisa el reemplazo que solicite el gestor de paquetes.

Las credenciales, claves de emparejamiento, perfiles de navegador, historiales, documentos, cachés y ejecutables compilados se conservan fuera del repositorio. Las cuentas deben autenticarse en el equipo restaurado. Para Bluetooth, empareja los periféricos y configura sus direcciones en archivos locales si hace falta.

## Wallpaper Studio del notebook

Para incorporar funciones puntuales en el desktop, consulta el [índice de apps y sus guías de traslado](apps/README.md).

Esta copia incluye la vista previa grande animada de GIF, la paleta del monitor de recursos, los paneles de Waybar y la terminal Kitty, y la sincronización con SDDM y Hyprlock. Las modificaciones de esta copia se realizan en `laptop/apps/`.

Al aplicar un fondo, SDDM y Hyprlock reciben un PNG estático y sus colores. Para revisar la configuración de inicio ya guardada:

```bash
wallpaper-studio --sync-login
```

SDDM puede leer la copia pública en `/var/lib/wallpaper-studio/<uid>/`; el estado personal permanece en `~/.local/state/wallpaper-studio/`. El arranque y el cambio de monitores restauran el fondo desde Wallpaper Studio, con Waypaper como respaldo.

## Actualizar la captura desde el notebook

```bash
cd ~/arch-workstation/laptop
bash update.sh
```

El exportador prepara una copia temporal, captura configuraciones y paquetes, normaliza las rutas a `@HOME@`, comprueba secretos y sintaxis y valida Hyprland antes de reemplazar los archivos de esta carpeta. Mantiene las entradas Lua cuando el entorno activo todavía usa `hyprland.conf`. Si encuentra un marcador que identifica el equipo como desktop, se detiene. En una instalación anterior sin marcador, la entrada específica del notebook captura solamente esta carpeta.

El código de las apps y sus pruebas se conserva desde el repositorio: no se reconstruye a partir de los ejecutables instalados. Para actualizar las versiones fijadas de Craft, revisa también las URL, commits y SHA-256 de `packages/craft-releases.json`; el inventario Flatpak por sí solo no cambia esas recetas.

## Comprobar y probar

```bash
python3 scripts/check.py
bash scripts/test.sh
cargo test --locked --manifest-path apps/waybar-editor/Cargo.toml
cargo clippy --locked --manifest-path apps/waybar-editor/Cargo.toml --all-targets -- -D warnings
```

La comprobación con disponibilidad actual de paquetes requiere red y bases de pacman recientes:

```bash
bash install.sh --check
```

Las pruebas incluyen una instalación en un HOME temporal utilizando únicamente una copia de esta carpeta y comprueban que se preserven el estado personal de Wallpaper Studio y los datos de los editores. No sustituyen una reinstalación física ni verifican el desbloqueo mediante contraseña en otro equipo.

Los respaldos de configuraciones se guardan en `~/.local/state/dotfiles/backups/`. La vinculación de SDDM/Hyprlock crea además un respaldo `~/.local/state/wallpaper-studio/login-backup.XXXXXXXX/` y muestra su ruta.

## Waybar y el protocolo Lua de Hyprland 0.57

Waybar 0.15.0 no incorpora el soporte Lua del módulo nativo `hyprland/workspaces`: validar JSONC y CSS no verifica sus órdenes IPC al pulsar. El perfil del notebook usa `waybar-git` para conservar los iconos de aplicaciones, los workspaces persistentes y sus clics. El lanzador central usa `hyprctl eval` con `hl.exec_cmd` y reglas nativas por lanzamiento.

En una instalación existente con `yay`, desde una terminal de Hyprland:

```sh
bash ~/arch-workstation/laptop/scripts/upgrade-waybar-lua.sh
```

Este comando verifica el soporte en el binario, instala la versión compatible si falta y reemplaza el proceso de Waybar; recargar con SIGUSR2 conserva el ejecutable anterior. No reemplaza la configuración de la barra.
