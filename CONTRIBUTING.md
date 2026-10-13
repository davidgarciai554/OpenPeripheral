# Contribuir

Gracias por ayudar. El objetivo es seguir siendo **pequeño, local y seguro**: sin cuentas, sin telemetría, sin más red que la descarga opcional de fotos de ratones y sin dependencias innecesarias.

## Antes de abrir un issue o PR

- **No pegues datos personales:** números de serie, direcciones MAC, nombres de dispositivos Bluetooth («Auriculares de …»), rutas como `C:\Users\<nombre>` ni volcados HID completos sin revisar. Sustitúyelos por `XX`.
- Indica el modelo, el `VID:PID` (lo muestra el Administrador de dispositivos) y la salida de `openperipheral --list`.

## Añadir un dispositivo

1. **Solo comandos documentados o verificados.** Cada constante nueva debe tener su procedencia en `docs/protocolos/<marca>.md`: especificación pública, documentación de terceros o captura propia. Se puede portar código con licencia compatible con la AGPL-3.0-or-later del proyecto (MIT, BSD, Apache-2.0, GPL-2.0-or-later, GPL-3.0 o AGPL-3.0, como OpenMouse) si se cumplen estas condiciones:
   - **(a) Procedencia comprobada en cada módulo.** Se excluye lo que derive de software del fabricante (por ejemplo, el Ninjutso de OpenMouse, sacado del panel web de NinjaForce) y lo que tenga un origen dudoso.
   - **(b) Cita en `docs/protocolos/<marca>.md`:** repositorio, commit y licencia.
   - **(c) Cabecera en el `.rs` portado** con el origen, el copyright y la licencia.
   - **(d) Cumplir las reglas 2 a 7**, aunque el original haga otra cosa. Por ejemplo, se quitan sus escrituras persistentes.

   El código con licencia incompatible (por ejemplo, GPL-2.0 *only*) sirve para entender el protocolo, pero hay que reimplementarlo.
2. **Nada persistente, salvo el perfil integrado verificado.** Las escrituras deben ser volátiles. Están prohibidos el firmware, DFU y bootloader y la flash. La única excepción es el perfil integrado de un ratón cuyo formato esté verificado en hardware (hoy, el G305 y el G203 LIGHTSYNC), y solo a través de `ProfileImage` (`src/protocol/onboard.rs`): confirmación del usuario, copia original, relectura y vuelta atrás si falla.
3. **Códec puro en `src/protocol/`.** Las peticiones se definen como `enum`. No se puede construir un comando que no esté en el enum. Hay que validar longitud, identificador e integridad de cada respuesta, sin indexar a ciegas.
4. **Controlador en `src/devices/`** que implemente `Device` usando solo `Transport`.
5. **Entrada en `SUPPORTED`** (`src/devices/mod.rs`) con la página y el uso HID exactos, y la interfaz si hace falta. Los ratones Logitech HID++ 2.0 ya se detectan sin entrada (`AUTO_LOGITECH`, solo ajustes volátiles). Una entrada propia hace falta para verificar el modelo, por ejemplo para habilitar su perfil integrado (`onboard: true`, regla 2).
6. **Pruebas sin hardware:** vectores de ejemplo (sintéticos o capturas saneadas), casos límite y respuestas malformadas, usando `FakeTransport`.
7. **Prueba en hardware real:** leer, cambiar, releer y restaurar. Anota el resultado en el PR y añade el dispositivo a la tabla del README.

## Comprobaciones

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --locked
```

Al contribuir aceptas que tu aportación se publique bajo la licencia AGPL-3.0-or-later del proyecto. No se pide CLA.
