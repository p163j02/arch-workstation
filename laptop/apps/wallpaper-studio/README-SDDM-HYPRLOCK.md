# Fondo y paleta del acceso y del bloqueo del notebook

Esta integración pertenece exclusivamente a la copia de Wallpaper Studio situada en `laptop/apps/`. Al elegir **Aplicar fondo y paleta** actualiza **SDDM/Pixie** para el inicio de sesión y **Hyprlock** para el botón **Bloquear** y Super+L.

Desde `laptop/`, con las dependencias de la app ya instaladas:

```bash
bash apps/wallpaper-studio/install.sh
bash apps/wallpaper-studio/setup-login-theme.sh
```

La instalación completa del notebook ya ejecuta estos pasos. La configuración inicial comprueba que el binario admita `--sync-login`, respalda Pixie y Hyprlock, conserva las traducciones del tema instalado y utiliza `laptop/hypr/hyprlock.conf`. Necesita `sudo` una vez por equipo. Las aplicaciones posteriores del fondo se realizan como el usuario normal.

El código está en `../waybar-editor/src/wallpaper/login.rs`. `theme.rs` lo llama al aplicar el fondo; `sddm_theme.py` vincula los colores de Pixie sin alterar el flujo de autenticación. Los recursos del tema están en `../../sddm/` y los del bloqueo en `../../hypr/`.

Los archivos exportados `background.png` y `theme.conf` se guardan con permisos de lectura para SDDM en `/var/lib/wallpaper-studio/<uid>/`. El archivo `theme.conf.user` del tema apunta a esa configuración. Los originales y el estado personal conservan sus permisos. Los GIF utilizan un fotograma estático en estas dos pantallas.

Para resincronizar únicamente el inicio y el bloqueo con el fondo ya guardado:

```bash
wallpaper-studio --sync-login
```

Si aún no has elegido un fondo, abre la app, añade tu carpeta y aplica uno. El instalador conserva un estado personal anterior; los valores incluidos en `laptop/state/` solo inicializan un equipo que todavía no lo tenga.

Las pruebas del notebook cubren colores, permisos y archivos generados. El tema se inspeccionó con el modo de prueba de SDDM y la configuración con el parser de Hyprlock. La comprobación de arranque y desbloqueo reales queda para el equipo restaurado. Consulta [la guía del notebook](../../README.md) para instalación y respaldos.


## Incorporar esta funcionalidad al desktop

Este procedimiento es una guía para una modificación posterior del perfil desktop. La documentación no aplica cambios al equipo ni al código de desktop. Ejecuta los comandos de copia desde la raíz de `arch-workstation`, después de guardar tus cambios locales en Git. Revisa cada diferencia para conservar mejoras que ya tenga tu desktop.

### 1. Incorporar el código y sus pruebas

En la revisión `a42e5a8`, estos archivos forman la extensión completa de Wallpaper Studio del notebook, incluida la corrección de la vista previa GIF:

```bash
cp laptop/apps/waybar-editor/src/wallpaper.rs desktop/apps/waybar-editor/src/wallpaper.rs
cp laptop/apps/waybar-editor/src/wallpaper/theme.rs desktop/apps/waybar-editor/src/wallpaper/theme.rs
cp laptop/apps/waybar-editor/src/wallpaper/login.rs desktop/apps/waybar-editor/src/wallpaper/login.rs
mkdir -p desktop/apps/waybar-editor/tests/fixtures
cp laptop/apps/waybar-editor/tests/fixtures/preview.gif desktop/apps/waybar-editor/tests/fixtures/preview.gif
cp laptop/apps/wallpaper-studio/setup-login-theme.sh desktop/apps/wallpaper-studio/
cp laptop/apps/wallpaper-studio/sddm_theme.py desktop/apps/wallpaper-studio/
```

Si ambas copias han evolucionado, incorpora los cambios con un diff en lugar de sobrescribir archivos. La extensión requiere conjuntamente:

- `wallpaper.rs`: `mod login`, opción `--sync-login` y ayuda CLI.
- `wallpaper/theme.rs`: sincronización tras aplicar y escritura atómica con permisos explícitos.
- `wallpaper/login.rs`: exportación del fotograma/colores y actualización del bloque de Hyprlock.
- `setup-login-theme.sh` y `sddm_theme.py`: preparación de la ruta pública y vinculación de Pixie.

Los `include_str!` de pruebas resuelven archivos dentro del mismo perfil: `desktop/hypr/hyprlock.conf` y `desktop/waybar/style.css`. No deben apuntar a `laptop/`. El script copiado ya resuelve `../../hypr/hyprlock.conf` desde su propia carpeta.

### 2. Adaptar la plantilla de Hyprlock del desktop

Edita `desktop/hypr/hyprlock.conf`, conservando las posiciones, fuentes, tamaños y monitores del desktop. Añade exactamente un bloque administrado al principio; la imagen inicial debe existir en ese equipo:

```ini
# wallpaper-studio:begin
$ws_wallpaper = ~/wallpapers/dmc-9.png
$ws_accent = rgba(7aa2f7ee)
$ws_secondary = rgba(bb9af7ee)
$ws_background = rgba(1a1b26ee)
$ws_text = rgba(c0caf5ff)
# wallpaper-studio:end
```

Cambia las propiedades de los bloques existentes según esta tabla. Son valores de la plantilla, no bloques adicionales:

| Bloque / propiedad | Valor |
| --- | --- |
| `background.path` | `$ws_wallpaper` |
| `label.color` para horas | `$ws_accent` |
| `label.color` para minutos | `$ws_secondary` |
| `label.color` para fecha y texto | `$ws_text` |
| `input-field.outer_color` | `$ws_accent` |
| `input-field.inner_color` | `$ws_background` |
| `input-field.font_color` | `$ws_text` |
| `input-field.check_color` | `$ws_secondary` |

El color de contraseña incorrecta puede conservar su rojo de alarma. Usa [la plantilla del notebook](../../hypr/hyprlock.conf) como referencia de sintaxis; no es necesario trasladar su diseño completo. Tras sincronizar, la app sustituye solo el bloque administrado y conserva el resto de la plantilla activa.

### 3. Compilar y preparar el equipo desktop

Requisitos: las dependencias de compilación de [Waybar Editor](../waybar-editor/README.md#compilar-e-instalar), `awww` con su daemon activo, Hyprlock y SDDM con **Pixie** instalado y seleccionado. Esta integración espera `/usr/share/sddm/themes/pixie/Main.qml`, `components/Clock.qml` y `theme.conf`; no adapta otros temas de SDDM.

Desde la raíz del repositorio:

```bash
cargo test --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml
cargo clippy --locked --manifest-path desktop/apps/waybar-editor/Cargo.toml --all-targets -- -D warnings
bash desktop/apps/wallpaper-studio/install.sh
wallpaper-studio --help
```

Comprueba que la ayuda incluya `--sync-login`. Abre **Wallpaper Studio** y aplica un fondo del desktop antes de ejecutar el siguiente paso; así existe una selección inicial válida en su estado personal.

```bash
bash desktop/apps/wallpaper-studio/setup-login-theme.sh
wallpaper-studio --sync-login
```

**El script sustituye el `~/.config/hypr/hyprlock.conf` activo por la plantilla de `desktop/hypr/hyprlock.conf`, con respaldo previo.** Por eso debes adaptar primero la plantilla del desktop. No basta con editar únicamente el archivo instalado. El script vincula Pixie usando `sudo`, sin reiniciar SDDM ni cerrar sesión; después, aplicar un fondo sincroniza ambas pantallas como usuario normal.

Para que futuras instalaciones completas del desktop lo activen automáticamente, incorpora la llamada a su propio `setup-login-theme.sh` en el flujo de instalación de desktop, después de instalar el binario y desplegar Pixie. No sustituyas el instalador del desktop por el del notebook. Si reinstalas la plantilla de Hyprlock, vuelve a ejecutar la preparación para dejarla sincronizada.

### 4. Comprobar el resultado y recuperar

```bash
readlink ~/.local/state/wallpaper-studio/login-theme
readlink /usr/share/sddm/themes/pixie/theme.conf.user
```

Con `XDG_STATE_HOME` personalizado, usa ese directorio en lugar de `~/.local/state`. Los enlaces deben resolver al directorio `/var/lib/wallpaper-studio/<uid>/` y su `theme.conf`, respectivamente. Verifica que existan `background.png` y `theme.conf`, y que el bloque de Hyprlock apunte al PNG exportado.

Aplica otra imagen/paleta y verifica el cambio de archivos, el botón **Bloquear** y, cuando puedas cerrar sesión, el acceso de SDDM. Hyprlock bloquea una sesión ya abierta; SDDM muestra el acceso previo a la sesión. Los GIF se previsualizan animados en la app y se exportan como fotograma estático para estas pantallas.

La preparación muestra un respaldo `login-backup.XXXXXXXX/` dentro del estado de Wallpaper Studio. Para revertir, restaura `hyprlock.conf` como usuario y `Main.qml`/`Clock.qml` en Pixie con `sudo`. Si el respaldo contiene `theme.conf.user`, retira primero el enlace administrado y restaura ese archivo; si no existía antes, retira solamente ese enlace para volver al `theme.conf` base. Conserva el respaldo hasta verificar un inicio y desbloqueo reales.

No copies el estado, fondos ni configuración completa de monitores del notebook. El directorio público contiene únicamente el PNG y sus colores; el tema de SDDM queda vinculado al usuario que ejecutó la preparación, incluso si hay varias cuentas.
