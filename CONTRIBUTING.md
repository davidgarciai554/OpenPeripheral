# Contribuir

Gracias por ayudar. El objetivo es seguir siendo **pequeño, local y seguro**: sin red, sin cuentas, sin dependencias innecesarias.

## Antes de abrir un issue o PR

- **No pegues datos personales:** números de serie, direcciones MAC, nombres de dispositivos Bluetooth («Auriculares de …»), rutas como `C:\Users\<nombre>` ni volcados HID completos sin revisar. Sustitúyelos por `XX`.
- Indica el modelo, el `VID:PID` (lo muestra el Administrador de dispositivos) y la salida de `openperipheral --list`.

## Añadir un dispositivo

1. **Solo comandos documentados o verificados.** Cada constante nueva debe tener su procedencia en `docs/protocolos/<marca>.md`: especificación pública, documentación de terceros o captura propia. No copies código de proyectos GPL o AGPL (OpenRazer, OpenRGB, OpenMouse…). Puedes usarlos para entender el protocolo, pero hay que reimplementar.
2. **Nada persistente.** Las escrituras deben ser volátiles. Están prohibidos el firmware, DFU y bootloader, la flash y los perfiles integrados.
3. **Códec puro en `src/protocol/`.** Las peticiones se definen como `enum`. No se puede construir un comando que no esté en el enum. Hay que validar longitud, identificador e integridad de cada respuesta, sin indexar a ciegas.
4. **Controlador en `src/devices/`** que implemente `Device` usando solo `Transport`.
5. **Entrada en `SUPPORTED`** (`src/devices/mod.rs`) con la página y el uso HID exactos, y la interfaz si hace falta.
6. **Pruebas sin hardware:** vectores de ejemplo (sintéticos o capturas saneadas), casos límite y respuestas malformadas, usando `FakeTransport`.
7. **Prueba en hardware real:** leer, cambiar, releer y restaurar. Anota el resultado en el PR y añade el dispositivo a la tabla del README.

## Comprobaciones

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
```

Al contribuir aceptas que tu aportación se publique bajo la doble licencia MIT o Apache-2.0 del proyecto.
