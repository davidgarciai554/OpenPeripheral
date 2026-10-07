# Logitech HID++ 2.0

Implementación: `src/protocol/hidpp.rs`, `src/protocol/onboard.rs` (perfiles integrados), `src/devices/logitech.rs`.

## Fuentes

- Especificaciones públicas de features HID++ 2.0 publicadas por Logitech: IRoot 0x0000, DeviceName 0x0005, BatteryStatus 0x1000, UnifiedBattery 0x1004, AdjustableDPI 0x2201, ReportRate 0x8060 y OnboardProfiles 0x8100.
- Observación propia el 2026-10-06 con un G305 Lightspeed en un receptor LIGHTSPEED 046D:C53F: HID++ 4.2, índice de dispositivo 1.

## Transporte

- Interfaz HID de página `0xFF00`, uso `0x0002` (informes largos, id `0x11`, 20 bytes).
- Petición: `[0x11, índice, índice_feature, (función << 4) | sw_id, parámetros…]`.
- `sw_id = 0x0B`. Otros programas (G HUB, OpenLogi) usan otros valores, y las respuestas se emparejan por índice, feature, función y `sw_id`.
- Error HID++ 2.0: `[0x11, índice, 0xFF, índice_feature, función|sw_id, código]`. Error HID++ 1.0: `[0x10, índice, 0x8F, …]`.
- Índices: `0xFF` para un dispositivo directo y `1–6` para dispositivos tras un receptor. Se prueban todos con un ping (IRoot fn1); solo cuentan los que devuelven el byte de ping y una versión ≥ 2.

## Funciones usadas

| Feature | Función | Uso | Escribe |
| --- | --- | --- | --- |
| 0x0000 | fn0 getFeature, fn1 ping | Resolver índices y detectar dispositivos | No |
| 0x0005 | fn0 longitud, fn1 trozo | Nombre del modelo | No |
| 0x1000 | fn0 | Batería: %, siguiente nivel, estado (0 descarga; 1, 2, 4 cargando; 3 completa) | No |
| 0x1004 | fn1 | Batería unificada: %, nivel, estado (0 descarga; 1, 2 cargando; 3 completa) | No |
| 0x2201 | fn1 lista, fn2 actual | DPI permitido y actual | No |
| 0x2201 | fn3 setSensorDpi | Cambiar DPI. **Volátil**: no toca los perfiles en flash. | Sí |
| 0x8060 | fn0 lista, fn1 actual | Tasa de sondeo, solo lectura | No |
| 0x8100 | fn0 getInfo, fn4 getCurrentProfile, fn5 memoryRead | Leer el perfil integrado activo, el directorio y la copia de fábrica | No |
| 0x8100 | fn6 memoryWriteStart, fn7 memoryWrite, fn8 memoryWriteEnd | Guardar los botones en el perfil de usuario activo. **Persistente**: escribe en la memoria del ratón | Sí |

Lista de DPI: valores u16 big-endian terminados en 0. Si un valor tiene los tres bits altos a 1 (`0xE000`), indica un paso (`valor & 0x1FFF`) y forma el rango `[mín, paso, máx]`.

## Verificado en hardware (G305)

- Batería 90 %, coherente con G HUB.
- DPI 1000 → 1600 → releído 1600 → restaurado 1000.
- `getOnboardMode` (0x8100 fn2) devuelve `1`, el modo perfiles integrados. En este modo `setReportRate` (0x8060 fn2) se rechaza con error HID++ 2.0. **No se usa:** cambiar a modo host (`setOnboardMode 2`) queda fuera de v0.1 porque no se ha verificado si persiste.

## Perfiles integrados (0x8100)

Solo se interpreta lo observado en un G305 (formato de perfil 3). Cualquier otro formato se trata como «no compatible» y no se lee ni se escribe.

- **getInfo (fn0):** byte 1 formato de perfil, byte 3 número de perfiles, byte 5 número de botones y bytes 7–8 tamaño de sector (u16 big-endian). G305: formato 3, 1 perfil, 6 botones y sectores de 255 bytes.
- **getCurrentProfile (fn4):** sector del perfil activo (u16 big-endian). G305: `0x0001`.
- **memoryRead (fn5):** parámetros sector (u16) y desplazamiento (u16); devuelve 16 bytes. Leer más allá del final del sector se rechaza, así que la última lectura se alinea a `tamaño − 16`.
- **Escritura:**
  - `memoryWriteStart` (fn6) recibe sector, desplazamiento 0 y longitud del sector (tres u16).
  - Luego se envían trozos `memoryWrite` (fn7) de 16 bytes; el último se rellena con `0xFF`.
  - Termina con `memoryWriteEnd` (fn8).
  - Cada petición espera su respuesta.
- **Sectores:**
  - `0x0000`: directorio. Entradas de 4 bytes `[sector u16, activo, 0]` terminadas en `FF FF`.
  - `0x0001…`: perfiles de usuario.
  - `0x0101…`: copia de fábrica del perfil correspondiente (ROM; observado `0x0001 → 0x0101`).
- **CRC:** CRC-CCITT (polinomio 0x1021, inicio 0xFFFF) de los bytes `0..n−2`, guardado en big-endian en los dos últimos bytes del sector. Comprobado con el directorio y con los perfiles de usuario y de fábrica.
- **Botones (formato 3):** desde el byte `0x20`, una entrada de 4 bytes por botón.
  - `80 01 hi lo`: botones de ratón (máscara: 0x01 izquierdo, 0x02 derecho, 0x04 central, 0x08 atrás, 0x10 adelante).
  - `80 02 mod uso`: tecla con bits modificadores HID y uso HID.
  - `90 xx`: función del perfil (observado `90 05` en fábrica y `90 0C` en el perfil de usuario; su significado no está verificado).
  - Los demás bytes del perfil (DPI, tasa y los campos 0xD0–0xE5) no se interpretan y se conservan sin tocar.

### Reglas de escritura de la aplicación

- La única escritura posible es un `ProfileImage`.
  - Se construye a partir de una lectura recién hecha.
  - Solo cambia los 4 bytes de cada botón editado y el CRC.
  - Solo va al sector de un perfil de usuario listado en un directorio con CRC válido; nunca al `0x0000` ni a `0x01xx`.
  - El botón izquierdo no se puede cambiar.
- Solo se asignan codificaciones observadas: clics izquierdo, derecho y central, atrás, adelante y atajos de teclado. Doble clic, multimedia y «desactivar» no se ofrecen hasta tener evidencia de su formato.
- Antes de la primera escritura se guarda una copia original del perfil en `%APPDATA%\OpenPeripheral\backups\`. Esa copia nunca se sobrescribe y no lleva número de serie.
- Tras escribir se relee el sector entero. Si no coincide, se reescriben los bytes que había antes.
- No se escribe con G HUB en ejecución (`lghub.exe`, `lghub_agent.exe`; el servicio `lghub_updater` no cuenta) ni sin confirmación del usuario.
- No se envía `setOnboardMode`.

### Verificado en hardware (G305, 2026-10-07)

1. **Reescritura idéntica:** se escribió el perfil `0x0001` con sus mismos bytes y se releyó; los sectores 0x0000, 0x0001 y 0x0101 quedaron idénticos a la copia.
2. **Cambio y vuelta:**
   - Se reasignó un botón lateral y el usuario comprobó físicamente que ejecutaba el nuevo comando. No se envió `setOnboardMode` ni `setCurrentProfile`. No consta si hizo falta reconectar el ratón para que el cambio surtiera efecto.
   - Después se restauró la copia original, y los tres sectores releídos coinciden byte a byte con ella.

