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
