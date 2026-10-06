# OpenPeripheral

Control ligero de periféricos para Windows: batería, DPI y color, **sin cuentas, sin conexión a internet y sin telemetría**. Es una alternativa mínima a Razer Synapse, Logitech G HUB y similares para lo más habitual.

> *English:* lightweight, account-free peripheral control for Windows (battery, DPI, lighting). No network access, no telemetry. Single ~0.5 MB native binary.

## Qué hace

- Muestra la batería de los dispositivos compatibles en la bandeja del sistema, como texto: porcentaje y estado de carga.
- La ventana **Dispositivos** muestra una tarjeta por dispositivo: tipo, conexión y estado, una pila de 5 franjas con el porcentaje (o «Con cable»), DPI, tasa de sondeo y el último color aplicado. Sigue el tema claro u oscuro de Windows, y el alto contraste.
- Cambia el DPI y el color de los dispositivos que lo admiten, por ahora **solo desde la línea de órdenes**: el botón «Ajustes» de cada tarjeta aún no tiene pantalla. Los cambios son **volátiles**: no se escriben en la memoria del dispositivo.
- Avisa si hay otro software de periféricos en ejecución que pueda sobrescribir los ajustes, como G HUB, Synapse, Steam Input u OpenLogi. Nunca lo cierra.

## Dispositivos verificados

Solo se listan dispositivos probados en hardware real. Otros modelos pueden funcionar, pero no se abren hasta añadirlos a la lista blanca (ver [CONTRIBUTING.md](CONTRIBUTING.md)).

| Dispositivo | Conexión | Batería | DPI | Color | Notas |
| --- | --- | --- | --- | --- | --- |
| Logitech G305 Lightspeed | Receptor LIGHTSPEED (046D:C53F) | Sí | Sí | — | Tasa de sondeo solo de lectura (ver abajo) |
| Razer BlackWidow V3 Tenkeyless | USB (1532:0A24) | — | — | Estático | Synapse puede sobrescribir el color |
| Sony DualSense | USB (054C:0CE6) | Sí | — | Lightbar | Bluetooth no admitido todavía |

**Pendiente de verificar:** Razer BlackShark V2 Pro (2020) y la batería de dispositivos Bluetooth.

## Uso

Descarga `openperipheral.exe` de la página de *Releases*, comprueba su suma SHA-256 y ejecútalo. No necesita instalación ni permisos de administrador. Windows SmartScreen puede avisar porque el binario no está firmado.

- **Clic o Enter** sobre el icono de la bandeja: abre la ventana Dispositivos. En ella, Tab recorre los botones y Esc la cierra.
- **Clic derecho o Mayús+F10**: menú con batería, «Abrir OpenPeripheral», «Actualizar ahora», «Iniciar con Windows» y «Salir».
- Con el teclado, **Win+B** lleva el foco a la bandeja.

Línea de órdenes (diagnóstico y ajustes; no guarda nada):

```text
openperipheral --list
openperipheral --apply 1 dpi=1600
openperipheral --apply 0 color=#FF8000
```

La configuración se guarda en `%APPDATA%\OpenPeripheral\config.toml`. Identifica cada dispositivo solo por `VID:PID/modelo`, sin números de serie.

## Limitaciones conocidas

- **Solo Windows 10/11.**
- **Tasa de sondeo del G305:** en modo «perfiles integrados» el ratón rechaza el cambio. Cambiar a modo host no está verificado como no persistente, así que en esta versión solo se muestra.
- Si otro programa controla el mismo dispositivo, puede deshacer los cambios.
- Dos dispositivos del mismo modelo conectados a la vez se muestran como uno.
- El botón «Ajustes» de cada tarjeta todavía no hace nada: la pantalla de ajustes está pendiente de diseño. Mientras tanto, DPI y color se cambian con `--apply`.
- Si la ventana es demasiado pequeña para mostrar todas las tarjetas, la barra de desplazamiento de Windows aparece clara también en el tema oscuro: Windows solo la oscurece con API no documentadas, que no se usan.
- Para lectores de pantalla, cada tarjeta se lee como un único texto («G305… Nivel de batería: 5 de 5, 90 %…»), no campo a campo.

## Compilar

Requiere Rust estable y las herramientas de compilación de Visual Studio (MSVC).

```text
cargo build --release --locked
cargo test
cargo clippy --all-targets -- -D warnings
```

## Seguridad y privacidad

- La aplicación no tiene código de red ni dependencias de red, y no envía nada a ningún sitio.
- Solo abre interfaces HID de una lista blanca y solo puede enviar comandos tipados y documentados. No hay forma de mandar bytes arbitrarios, y no existe código para firmware, DFU ni escritura en flash.
- Funciona como usuario normal (manifiesto `asInvoker`).

## Licencia

Doble licencia, a tu elección: [MIT](LICENSE-MIT) o [Apache-2.0](LICENSE-APACHE).

Los protocolos se han reimplementado a partir de documentación pública y de observación propia. No incluye código de proyectos GPL como OpenRazer u OpenRGB. Logitech, Razer, Sony y sus productos son marcas de sus respectivos propietarios; este proyecto no está afiliado a ellos.
