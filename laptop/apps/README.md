# Apps del notebook y funciones para incorporar al desktop

Cada paquete tiene su README con funciones, archivos implicados, dependencias y pasos para trasladarlas a una copia independiente en `desktop/`.

| Paquete | Qué documenta | Guía |
| --- | --- | --- |
| Waybar Editor | Editor de barra, lanzadores, presets y relación del binario con Wallpaper Studio. | [README](waybar-editor/README.md#llevar-funcionalidades-al-desktop) |
| Wallpaper Studio | Fondos, paletas, vista previa grande animada de GIF y colores de Waybar/Eww. | [README](wallpaper-studio/README.md#llevar-funcionalidades-al-desktop) |
| Wallpaper Studio: SDDM/Hyprlock | Fondo y paleta al iniciar sesión y al bloquear, archivos a incorporar y recuperación. | [Procedimiento para desktop](wallpaper-studio/README-SDDM-HYPRLOCK.md#incorporar-esta-funcionalidad-al-desktop) |
| Craft | Restauración verificada de ArtCraft Launcher, GridCraft, PdfCraft y WordCraft. | [README](craft/README.md) |

Las referencias describen la captura del notebook de la revisión `a42e5a8`. Incorporar una función significa adaptar su código y configuración en `desktop/`, conservando los monitores, preferencias y datos de ese equipo. Las guías no crean dependencias entre perfiles ni ejecutan automáticamente las migraciones.

SDDM, Hyprlock y los widgets Eww son integraciones del entorno; sus archivos están en `../sddm/`, `../hypr/` y `../eww/`. El procedimiento de Wallpaper Studio explica cómo se conectan con la aplicación.
