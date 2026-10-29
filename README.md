# Arch Workstation — desktop y notebook

Instalación personal de Hyprland, Waybar, Eww, Fish, Kitty, editores y aplicaciones. Revisada el **28 de octubre de 2026** contra los repositorios oficiales de Arch y AUR. Cada equipo mantiene sus configuraciones, aplicaciones, paquetes y recursos. El notebook tiene además instalador, exportador y validaciones autónomos dentro de `laptop/`.

Esto configura un **Arch Linux ya instalado**. El particionado, cifrado, arranque, usuario y acceso a Internet deben funcionar antes. No modifica particiones ni el bootloader.

Para reproducir el notebook actual, consulta [la guía autónoma de laptop](laptop/README.md). Sus entradas principales son `bash laptop/install.sh` y `bash laptop/update.sh`.

## Instalar en un equipo nuevo

1. Instala Arch Linux x86_64 con un usuario normal que tenga acceso a `sudo`, conexión a Internet y un kernel `linux`, `linux-lts`, `linux-zen` o `linux-hardened`. NVIDIA requiere una tarjeta **Turing o posterior**: la RTX 2080 Super está dentro de este alcance. Los NVIDIA anteriores requieren otro controlador y no están cubiertos por el instalador.
2. Habilita `multilib` en `/etc/pacman.conf`:

   ```ini
   [multilib]
   Include = /etc/pacman.d/mirrorlist
   ```

3. Prepara las herramientas mínimas:

   ```bash
   sudo pacman -Syu --needed git python sudo
   ```

4. Clona el repositorio público:

   ```bash
   git clone https://github.com/p163j02/arch-workstation.git
   cd arch-workstation
   ```

5. Ejecuta **uno** de estos comandos como tu usuario, sin anteponer `sudo`:

   ```bash
   bash install.sh laptop
   ```

   ```bash
   bash install.sh desktop
   ```

6. Revisa los paquetes y las recetas AUR cuando se solicite, espera la confirmación final y reinicia. En SDDM selecciona **Hyprland**.

Los accesos anteriores `bash laptop/install.sh` y `bash desktop/install.sh` siguen funcionando desde cualquier directorio.

El instalador actualiza el sistema completo con `pacman -Syu`, instala las aplicaciones y los headers de los kernels detectados, prepara `yay` si falta, verifica los nombres de paquetes, instala las configuraciones y habilita los servicios para el próximo arranque. No reinicia SDDM, NetworkManager ni Bluetooth durante la sesión en curso. Incluye Steam, desarrollo, Docker, VirtualBox y TeX Live: la descarga requiere espacio y puede tardar bastante.

Detecta microcódigo AMD/Intel y las GPU AMD, Intel o NVIDIA compatibles. Instala `nvidia-open-dkms` para NVIDIA; VirtualBox utiliza DKMS. En notebook instala Bluetooth y compila la revisión de AirPods compatible con el panel. Se usa Node.js 24 LTS con Corepack/pnpm; Fish conserva su integración con nvm.fish.

La instalación completa de **ambos perfiles** también compila **Waybar Editor y Wallpaper Studio** desde el código Rust incluido en `desktop/apps/waybar-editor` o `laptop/apps/waybar-editor`, según el perfil, con las versiones de dependencias fijadas en `Cargo.lock`. Instala `~/.local/bin/waybar-editor`, `~/.local/bin/wallpaper-studio` y los dos accesos en Rofi. Requiere Internet para descargar las dependencias de Rust; no copia un binario del equipo original.

Si ya instalaste el entorno y solo quieres añadir el editor, desde el repositorio actualizado ejecuta como tu usuario:

```bash
sudo pacman -Syu --needed rust pkgconf gtk4 libadwaita python python-gobject python-cairo gtk3
bash desktop/apps/waybar-editor/install.sh
# En notebook: bash laptop/apps/waybar-editor/install.sh
~/.local/bin/waybar-editor
```

Abrir el editor no modifica la barra. Puedes editar aplicaciones, módulos y apariencia; al pulsar **Aplicar** se guarda un respaldo y se solicita a Waybar una recarga. El modo `--config-only` no compila ni instala el editor. Consulta [su documentación](desktop/apps/waybar-editor/README.md).

El mismo binario incluye **Wallpaper Studio**, integrado en el botón de fondos de Waybar de **laptop y desktop**. Selecciona fondos estáticos o GIF animados; cada imagen conserva su paleta para Waybar, los bordes de Hyprland, el monitor de recursos y los desplegables Eww. Para instalarlo o actualizarlo en un equipo que ya usa este repositorio, sigue [la guía de Wallpaper Studio](desktop/apps/wallpaper-studio/README.md). También tiene un acceso de instalación directo: `bash desktop/apps/wallpaper-studio/install.sh` (o `laptop/apps/wallpaper-studio/install.sh`).

## Comprobar sin instalar

En un Arch que ya tenga Hyprland, Fish, Lua y Python:

```bash
bash install.sh laptop --check
```

También valida desktop. Comprueba sintaxis, JSON/JSONC, enlaces relativos, credenciales literales, la configuración completa de ambos perfiles y la disponibilidad actual de paquetes oficiales/AUR. Requiere Internet para AUR y bases de pacman recientes; actualízalas mediante `sudo pacman -Syu`.

La comprobación local, sin consultar la red:

```bash
python3 scripts/check.py
```

Las pruebas automáticas, con Python, Fish, Lua, Hyprland, rsync, compilador C, GTK3, GtkLayerShell, libnm, pkgconf y json-glib instalados:

```bash
bash scripts/test.sh
```

Las pruebas instalan los archivos en directorios personales temporales. No instalan paquetes, no usan sudo, no cambian dispositivos y no arrancan una sesión gráfica.

## Aplicar únicamente los archivos de configuración

Si las aplicaciones ya están instaladas:

```bash
bash install.sh laptop --config-only
```

O usa `desktop`. Requiere Python, Fish, Lua, Hyprland, rsync y `cc`. Copia las configuraciones, servicios de usuario y fondos del perfil, y compila el cliente IPC local de Eww. En desktop también compila el adaptador CFFI de los workspaces; requiere `pkgconf`, GTK3 y `json-glib`. No instala paquetes, cambia la shell de inicio ni modifica `/etc`. Los servicios empiezan al abrir una nueva sesión de Hyprland.

Antes de reemplazar configuraciones se crea un respaldo privado en:

```text
~/.local/state/dotfiles/backups/FECHA-PID/
```

Los directorios están en `config/`, nano en `nanorc`, las unidades de usuario anteriores en la raíz del respaldo y los fondos reemplazados en `wallpapers/`. Una instalación completa también respalda los archivos de sistema y el tema Pixie anteriores. Restaura los archivos necesarios desde ese directorio con la sesión gráfica cerrada; los archivos de sistema requieren sudo.

Se conservan `hypr/local.conf`, `hypr/local.lua`, `*.local.*`, las variables universales de Fish y los archivos de autenticación existentes. Si un directorio completo de configuración es un enlace simbólico, el instalador pide resolverlo antes de reemplazarlo.

## Ajustes propios de cada máquina

Los monitores usan su modo preferido y Waybar admite todos los conectores. El notebook detecta el panel interno y la batería, sin exigir `eDP-1`, `BAT0` ni una resolución fija. `Super+F2` abre los modos de pantalla del notebook.

Para ajustes locales de monitores, teclado y los bordes de Wallpaper Studio, se conserva el archivo existente:

```text
~/.config/hypr/local.conf
```

Por ejemplo:

```ini
input {
    kb_layout = latam
}
# monitor = DP-1, preferred, auto, 1
```

Para atajos, reglas u otras personalizaciones en Lua, usa `~/.config/hypr/local.lua`; se carga al final y tampoco se exporta. El lector de `local.conf` admite ajustes de configuración y líneas `monitor`, conservando los bloques de colores de Wallpaper Studio.

El centro de control usa el único backlight disponible y detecta un monitor externo para DDC/CI. En desktop controla el monitor externo aunque no haya backlight de portátil; en NVIDIA puede detectar su bus mediante `ddcutil`. Con varios dispositivos, crea `~/.config/eww/control-center/settings.local.json` con la selección real del equipo:

```json
{
  "backlight_device": "intel_backlight",
  "external_connector": "DP-1",
  "wifi_interface": "",
  "bluetooth_adapter": "",
  "airpods_address": "AA:BB:CC:DD:EE:FF"
}
```

Omite las claves que no necesites. Selecciona la dirección real de tus AirPods después de emparejarlos; `bluetoothctl devices Paired` permite consultarla. No se copian claves de emparejamiento ni se conecta automáticamente a una dirección personal fija. Si el bus no puede asociarse al conector, añade `"ddc_bus": "7"` a la configuración local usando el número real que muestre `ddcutil detect --brief`. Para DDC/CI, el monitor debe admitirlo y tenerlo habilitado; `ddcutil` debe disponer del acceso I²C que configura su paquete. No se sustituye el brillo físico por gamma.

Los ajustes locales no se exportan. Guarda fuera del repositorio cualquier calibración privada que necesites recuperar.

## Mantener el respaldo actualizado

Después de instalar este formato, el equipo queda marcado como laptop o desktop. Exporta con el perfil correspondiente:

```bash
bash update.sh laptop
# En el otro equipo: bash update.sh desktop
```

El script prepara y valida una copia temporal antes de actualizar el repo. Sin un marcador de instalación o con un perfil incorrecto, se detiene. Actualiza **solo el perfil seleccionado**, incluidos sus scripts y fondos, conserva íntegro el otro perfil y normaliza las rutas personales a `@HOME@`. El instalador sustituye ese marcador por la ruta del nuevo usuario.

No exporta credenciales, `fish_variables`, `node_modules`, ajustes locales, binarios compilados, cachés, grafos generados ni archivos `.bak`/`.save`. Tampoco importa automáticamente cambios desde `/etc`, listas de paquetes o fuentes de aplicaciones; esos archivos se mantienen explícitamente dentro de cada perfil.

Revisa antes de guardar:

```bash
git diff --check
git diff --stat
bash scripts/test.sh
git add .
git commit -m "Actualizar dotfiles de Arch"
git push
```

El exportador también funciona desde una copia portable sin `.git`; en ese caso no muestra el estado de Git.

## Organización

```text
install.sh                 Instalador para el perfil seleccionado
update.sh                  Exportación exclusiva del perfil seleccionado
scripts/                   Validación, detección de hardware y pruebas
desktop/                   Configuración del PC de escritorio actual
  apps/waybar-editor/      Rust, Cargo.lock e instalador de ambas aplicaciones
  apps/wallpaper-studio/   Instalador y guía de Wallpaper Studio
  packages/               Listas propias de paquetes oficiales y AUR
  hypr/                   Configuración, sesión y atajos del escritorio
  waybar/                 Barra actual, scripts y fuentes del adaptador CFFI
  eww/                    Monitor de recursos y centro de control
  systemd/user/           Servicios del escritorio, incluidos los workspaces
  sddm/                   Tema Pixie propio
  system/                 Zram, swappiness y apariencia GSettings
  wallpapers/             Fondos propios
  fish/, kitty/, …        Resto de configuraciones del escritorio
laptop/                    Copia independiente con la misma estructura
  hypr/                   Incluye batería y modos de pantalla del notebook
  waybar/                 Barra del notebook, sin cambios del desktop
tests/                     Instalación aislada, respaldos y sincronización
```

Cada perfil contiene todos sus archivos: no existe una carpeta `shared` ni se toman recursos del otro equipo. Las aplicaciones conservan fuentes independientes y se compilan desde `PERFIL/apps/waybar-editor`; Wallpaper Studio utiliza el mismo binario dentro de ese perfil.

Desktop incorpora la configuración activa de este PC, incluidos los workspaces desplegables de Waybar, su servicio y los atajos de cierre por grupos. El adaptador CFFI se reconstruye al instalar, sin distribuir bibliotecas `.so`. Consulta [los workspaces del desktop](desktop/waybar/WORKSPACES.md). Las rutas personales se normalizan y se omiten credenciales, variables universales de Fish, ajustes locales y archivos generados. Laptop conserva su configuración anterior en archivos propios.

Los dos perfiles incluyen `hypr/hyprland.lua`, validado con **Hyprland 0.57.0**, instalado en este PC. Esa versión retiró la lectura de configuraciones Hyprlang; se conserva `hyprland.conf` como referencia del formato anterior. Los cambios nuevos de Hyprland se editan en Lua. `monitors.conf` y los ajustes de `local.conf` se leen mediante `local-settings.lua`, incluyendo los bordes que guarda Wallpaper Studio. [Nota oficial de la versión](https://hypr.land/news/update57/).

## Seguridad y límites de la restauración

Este repositorio comienza con un historial Git nuevo. No incluye el historial del repositorio anterior, contraseñas, claves de proveedores, archivos de autenticación ni la cuenta Azure del equipo original. Configura tus proveedores de OpenCode localmente. Las claves que hayan sido expuestas en otro lugar deben revocarse aunque este repositorio sea nuevo.

Antes de crear commits, activa la comprobación de credenciales:

```bash
git config core.hooksPath .githooks
python3 scripts/check.py --secrets-only
```

El hook revisa los archivos preparados para el commit, no solo los archivos del directorio de trabajo. Si tienes Gitleaks instalado también lo ejecuta, sin mostrar valores de secretos. Consulta [SECURITY.md](SECURITY.md).

No incluye documentos personales, claves SSH, credenciales, proyectos, sesiones de navegadores, instalaciones externas de Pi ni autenticación de aplicaciones. Oh My Fish y el entorno de Rust solo se cargan cuando existen; su ausencia no impide iniciar Fish. El tema opcional de Oh My Fish no se descarga automáticamente.

Los paquetes de AUR y las aplicaciones externas pueden cambiar o fallar al compilar. AirPods usa la revisión `8af4eb20cbc39c33983d70c0518753db13f0bbfa` de [annoyedmilk/airpods-tui](https://github.com/annoyedmilk/airpods-tui), compatible con el IPC de este panel; no se distribuye un ejecutable copiado del equipo original. Su compilación requiere acceso a GitHub y crates.io.

La verificación automática cubre archivos, protocolos y flujos aislados. El inicio gráfico, la suspensión, NVIDIA, DDC/CI y Bluetooth requieren una prueba en cada equipo real; no se han certificado mediante una reinstalación física.
