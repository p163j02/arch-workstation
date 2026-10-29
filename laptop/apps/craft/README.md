# Craft del notebook

Este paquete restaura las aplicaciones externas de Craft presentes en el notebook. Contiene un instalador y un manifiesto de versiones; no contiene el código fuente de esas aplicaciones ni sus documentos o cuentas.

## Apps incluidas

| App | Formato / versión fijada | Identificador o destino |
| --- | --- | --- |
| ArtCraft Launcher | Flatpak 0.3.2 | `ai.storyteller.artcraft-launcher` |
| ArtCraft Launcher | AppImage 0.8.1 | `~/.local/opt/artcraft-launcher.AppImage` |
| GridCraft | Flatpak 0.12.1 | `ai.storyteller.gridcraft` |
| PdfCraft | Flatpak 0.14.0 | `ai.storyteller.pdfcraft` |
| WordCraft | Flatpak 0.11.0 | `ai.storyteller.wordcraft` |

Ambas distribuciones de ArtCraft están incluidas porque ambas estaban instaladas en el notebook. Las URL, SHA-256 y commits Flatpak se registran en [craft-releases.json](../../packages/craft-releases.json). Son versiones de la captura, no una selección automática del último release.

## Instalar únicamente estas apps

Requiere Arch x86_64, Python 3.11 o posterior, Flatpak, FUSE 2 para el AppImage y conexión a Internet. La instalación completa del notebook ya prepara estas dependencias y ejecuta el instalador. Para ejecutarlo por separado desde la raíz del repositorio:

```bash
sudo pacman -Syu --needed python flatpak fuse2
python3 laptop/apps/craft/install.py
```

El instalador añade Flathub para el usuario, descarga los artefactos de los releases registrados y comprueba su SHA-256 antes de instalarlos. Omite los Flatpak que ya tienen el commit esperado y el AppImage cuyo contenido coincide. Un hash incorrecto detiene el proceso; las apps procesadas antes del error pueden haber quedado instaladas.

Los Flatpak se instalan con `--user`. Para abrirlos:

```bash
flatpak run ai.storyteller.artcraft-launcher
flatpak run ai.storyteller.gridcraft
flatpak run ai.storyteller.pdfcraft
flatpak run ai.storyteller.wordcraft
```

El AppImage puede abrirse directamente desde `~/.local/opt/artcraft-launcher.AppImage`. El instalador de Craft no crea su acceso de menú: el instalador completo del notebook añade los archivos de [local/](../../local/).

## Llevar esta instalación al desktop

Desde la raíz del repositorio, incorpora estos archivos a la copia independiente de desktop, revisando primero cualquier implementación propia que ya exista:

```bash
mkdir -p desktop/apps/craft
cp laptop/apps/craft/install.py desktop/apps/craft/install.py
cp laptop/apps/craft/README.md desktop/apps/craft/README.md
cp laptop/packages/craft-releases.json desktop/packages/craft-releases.json
```

El script obtiene el manifiesto desde `../../packages/` relativo a su propia ubicación, por lo que la copia de desktop no depende de `laptop/`. Ajusta los ejemplos y enlaces de este README al perfil desktop tras copiarlo. Prepara `python`, `flatpak` y `fuse2` en los paquetes oficiales de ese perfil y ejecuta:

```bash
python3 desktop/apps/craft/install.py
```

Para automatizarlo, añade esa llamada a la instalación de desktop después de instalar las dependencias. Conserva su selección de versiones; cambia URL y SHA-256 juntos al actualizar un release, y el commit esperado para cada Flatpak.

Si quieres también el acceso de menú del AppImage, adapta estos dos archivos al perfil desktop:

- `../../local/bin/artcraft-launcher`, que ejecuta el AppImage instalado.
- `../../local/share/applications/ai.storyteller.artcraft-launcher.desktop`, que registra el acceso.

Los archivos del repositorio contienen `@HOME@`: renderízalos con el normalizador del perfil desktop antes de instalarlos en `~/.local/`, conservando el permiso ejecutable del script. Copiarlos sin renderizar deja rutas inválidas. No copies `laptop/preferences/` ni perfiles completos para instalar estas aplicaciones.

## Comprobación y límites

```bash
flatpak info --user ai.storyteller.gridcraft
flatpak info --user ai.storyteller.pdfcraft
flatpak info --user ai.storyteller.wordcraft
flatpak info --user ai.storyteller.artcraft-launcher
```

Las pruebas del instalador están en [test_craft.py](../../tests/test_craft.py): verifican origen de descarga, rechazo de hash incorrecto, reemplazo atómico del AppImage y omisión de Flatpak coincidentes. Pueden trasladarse a `desktop/tests/` adaptando la raíz del perfil.

Este paquete no modifica la paleta de SDDM ni de Hyprlock: esa función pertenece a [Wallpaper Studio](../wallpaper-studio/README.md#llevar-funcionalidades-al-desktop). No sincroniza cuentas, documentos ni sesiones de Craft; la disponibilidad de los artefactos depende del proveedor.
