# Audio y Wi-Fi: controles ampliados

## Ubicación

- Audio: `widgets/volume.yuck`, `scripts/backend.py`, `eww.yuck`.
- Wi-Fi: `scripts/wifi-panel.py`, lanzado por `scripts/popup-manager.py`.
- Iconos Waybar: `~/.config/waybar/config.jsonc`, bloque `pulseaudio`.

El popup Wi-Fi usa GTK3/GtkLayerShell dentro del mismo centro de control y namespace `waybar-control-center`. Los demás paneles continúan en Eww. El renderer `widgets/wifi.yuck` permanece como referencia anterior; no es el popup que abre Waybar. Esto evita pasar contraseñas por la interpolación shell de los campos input de la versión instalada de Eww.

## Audio

- Engranaje: pregunta si abrir Pavucontrol o Pulsemixer (este último en Kitty). El selector y la aplicación se ejecutan independientes del timeout de Eww; no se cierran a los cinco segundos.
- Mono: crea exclusivamente el sink virtual `waybar_cc_mono` mediante `module-remap-sink`, mezclando izquierda/derecha con `channels=1 channel_map=mono master_channel_map=mono remix=yes`.
- Estéreo: restaura la salida física y elimina el módulo virtual propio.
- Se trasladan los streams existentes, excepto el stream interno del remap. Se conserva porcentaje y silencio al alternar. El volumen físico funciona al 100% durante mono; el control de usuario está en el sink virtual. Al volver a estéreo se copia su volumen/silencio a la salida física.
- Seleccionar otra salida desde el panel mientras mono está activo vuelve a crear mono sobre la salida nueva.
- El modo es de sesión; no se instala configuración permanente de PipeWire.

## Wi-Fi

- Apertura: se restaura el cliente IPC nativo (`fast-client`) hacia el despachador persistente. El panel se muestra antes de consultar las redes; NetworkManager se inicializa de forma asíncrona y las consultas nmcli se ejecutan en un worker. Durante la carga se muestra «Cargando redes…» y se bloquean acciones hasta tener tanto el estado como los metadatos de seguridad.
- Engranaje: pregunta si abrir NetworkManager (`nm-connection-editor`) o nmtui, la interfaz de terminal de NetworkManager, en Kitty.
- Al conectar aparece un spinner animado y «Conectando…». Si falla, muestra un mensaje y restaura el campo vacío para reintentar. Los errores de autenticación indican que la contraseña puede ser incorrecta (sin afirmar una causa que NetworkManager no confirma); también se informa de fallos de IP y tiempo de espera.
- Las redes WPA/WPA2/WPA3 Personal nuevas piden contraseña dentro del popup, con mostrar/ocultar, Conectar, Cancelar y Enter.
- Al pulsar una red Personal aparece inmediatamente el campo de contraseña, incluso si hay un perfil guardado. El botón «Usar guardada» activa ese perfil mediante D-Bus. Si falla se vuelve a ofrecer entrada de contraseña.
- Las redes abiertas se activan sin solicitar contraseña.
- Redes empresariales, WEP u otras autenticaciones avanzadas usan el editor de conexiones.
- La contraseña viaja a NetworkManager mediante D-Bus, nunca como argumento de shell, variable Eww, archivo temporal propio o log. NetworkManager guarda el perfil/secreto según su política habitual para permitir reconexiones.
- Cancelar limpia el campo. Escape y clic exterior cierran el popup. El teclado usa ON_DEMAND para poder escribir sin capturar el foco al abrir.
- Requiere GTK3, GtkLayerShell y libnm GI, disponibles en este sistema.

## Validación

Pruebas no destructivas: `python3 -m unittest discover -s tests -v`.
Se verificó apertura real en eDP-1, renderizado, y alternancia mono/estéreo conservando volumen y silencio. No se probó conexión con una contraseña real ni se cambió la red activa.
