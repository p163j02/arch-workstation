# Workspaces desplegables

El módulo izquierdo usa una sola instancia de los workspaces nativos de
Waybar, conservando sus botones e iconos. Cerrado muestra `1–6 `;
abierto muestra `1–6 α–ζ `.
Los workspaces α–ζ corresponden a 7–12; los nuevos desde 13 también
pertenecen a la sección desplegable y muestran su número.

- Automático: se abre al entrar en un workspace desde el 7, o al tener
  ventanas allí. Se cierra al estar fuera y no quedar ninguna ventana allí.
- Pulsar ``: abre y fija la sección hasta pulsar ``, incluso estando vacía.
- Pulsar ``: oculta la sección aunque tenga ventanas; no mueve ni cierra
  ninguna. Una nueva visita a un workspace desde el 7 reactiva el modo
  automático. Si se vacían todos y estás fuera, vuelve al modo automático.

Los botones se ordenan por número y 1–12 son persistentes. Un viewport
recorta la lista después del sexto botón al cerrar la sección; no depende del
filtro `ignore-workspaces`, que Waybar 0.15 no aplica al cargar las reglas
persistentes de Hyprland. Una sola instancia evita listas duplicadas.
La apertura y el cierre duran 220 ms y usan la misma curva suave. Un pequeño
adaptador CFFI observa el estado y anima únicamente el ancho del viewport,
sincronizado con el reloj de fotogramas de GTK. Las letras, los iconos y los
botones conservan su tamaño; no se anima la tipografía ni sus márgenes.
El módulo nativo conserva todos sus botones originales y su ordenación.
No se recarga el CSS ni se reinicia la barra al alternar.
El botón recibe sus cambios por un flujo continuo, sin sondeo cada segundo.
El estado manual se conserva si se reinicia la barra o el servicio en la misma sesión de Hyprland.
Las sesiones nuevas empiezan en modo automático.

El adaptador está en `cffi/workspace-animation.c`; para recompilarlo:

```sh
cd ~/.config/waybar
python3 cffi/build-animation.py
```

El compilador genera un nombre de biblioteca con su hash y actualiza
`module_path` en la configuración. Esto es necesario porque Waybar conserva
las bibliotecas CFFI cargadas al recargar su configuración: volver a usar el
mismo nombre puede ejecutar una implementación anterior aunque se recompile.
El adaptador también guarda `animation-layout.json` junto al estado de la
sesión para comprobar el ancho solicitado y el realmente asignado por GTK.

## Atajos de cierre

| Combinación | Ventanas a cerrar |
| --- | --- |
| Super + Ctrl + Shift + F1 | Workspaces 1–6 |
| Super + Ctrl + Shift + F2 | Workspaces 7 en adelante |
| Super + Ctrl + Shift + F3 | Todos, incluido el scratchpad especial |

Se envía una solicitud normal de cierre a cada ventana, usando su dirección
de Hyprland. Las aplicaciones pueden pedir guardar cambios o cancelar el
cierre. No se matan procesos ni se cambia de workspace. Estos atajos actúan
directamente al pulsarlos.

Para consultar qué ventanas seleccionaría un grupo sin cerrarlas:

```sh
~/.config/hypr/close-workspaces.py main --dry-run
~/.config/hypr/close-workspaces.py extra --dry-run
~/.config/hypr/close-workspaces.py all --dry-run
```

## Activar en una sesión que ya está abierta

El inicio de sesión de Hyprland arranca el servicio automáticamente. Para
activar los cambios ahora, ejecutar desde una terminal de la sesión gráfica:

```sh
systemctl --user daemon-reload
systemctl --user import-environment HYPRLAND_INSTANCE_SIGNATURE WAYLAND_DISPLAY
systemctl --user restart waybar-workspaces.service
hyprctl reload
pkill -SIGUSR2 -x waybar
```

El último comando recarga una Waybar que ya está ejecutándose. Si no hay
ninguna, iniciarla con `waybar`.

Diagnóstico y pruebas sin cerrar ventanas:

```sh
journalctl --user -u waybar-workspaces.service -n 30
python3 ~/.config/waybar/scripts/test_workspace_section.py
```

Los archivos anteriores a este cambio están respaldados en
`~/.local/state/waybar-workspaces/backups/`.
