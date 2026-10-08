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
  - Solo cambia el ciclo de DPI (`0x03–0x0A`, el mismo valor en las 4 entradas), el bloque de luz activo (`0xDB–0xE5`), los 4 bytes de cada botón editado y el CRC. «Valores de fábrica» copia esos mismos rangos de la ROM. El DPI tiene que estar en el rango que da `0x2201`, el efecto entre los que declara `0x8070` y la velocidad en las tablas cerradas.
  - Solo va al sector de un perfil de usuario listado en un directorio con CRC válido; nunca al `0x0000` ni a `0x01xx`.
  - El botón izquierdo no se puede cambiar.
- Solo se asignan codificaciones observadas: clics izquierdo, derecho y central, atrás, adelante y atajos de teclado. Doble clic, multimedia y «desactivar» no se ofrecen hasta tener evidencia de su formato.
- Antes de la primera escritura se guarda una copia original del perfil en `%APPDATA%\OpenPeripheral\backups\`. Esa copia nunca se sobrescribe y no lleva número de serie.
- Tras escribir se relee el sector entero. Si no coincide, se reescriben los bytes que había antes.
- No se escribe con G HUB en ejecución (`lghub.exe`, `lghub_agent.exe`; el servicio `lghub_updater` no cuenta) ni sin confirmación del usuario. La confirmación es el diálogo de guardado o, para «Usar» un perfil (plan `perfiles-raton`), haber guardado ese perfil y pulsar «Usar»: el worker construye la imagen desde el perfil validado de la configuración, nunca desde cambios sueltos (`profiles::ProfileApply`).
- Perfiles de la app: el G305 tiene 1 perfil integrado, así que los perfiles con nombre se guardan en `config.toml` por modelo (`VID:PID/modelo`, sin número de serie); dos ratones iguales los comparten. «Usar» escribe solo el DPI (los 4 niveles), el bloque de luz y los botones asignables; las funciones del perfil (`90 xx`, botón DPI) no se tocan. Si el sector releído ya coincide con el perfil, no se escribe.
- No se envía `setOnboardMode`.

### Verificado en hardware (G305, 2026-10-07)

1. **Reescritura idéntica:** se escribió el perfil `0x0001` con sus mismos bytes y se releyó; los sectores 0x0000, 0x0001 y 0x0101 quedaron idénticos a la copia.
2. **Cambio y vuelta:**
   - Se reasignó un botón lateral y el usuario comprobó físicamente que ejecutaba el nuevo comando. No se envió `setOnboardMode` ni `setCurrentProfile`. No consta si hizo falta reconectar el ratón para que el cambio surtiera efecto.
   - Después se restauró la copia original, y los tres sectores releídos coinciden byte a byte con ella.
3. **Perfiles de la app («Usar», 2026-10-07, con permiso del usuario):** cambio entre «Perfil 1» (respiración azul) y «Juego» (1600 DPI, fija roja) desde la página y desde la bandeja. Cada cambio se verificó por relectura en unos 2–3 s (medido desde la interfaz, con 0,5–1 s de sondeo de la prueba). La luz cambió al momento y se mantuvo tras apagar y encender el ratón. Con el ratón apagado, «Usar» falló en la lectura previa y no escribió nada.


## Iluminación (0x8070) y DPI del perfil (observación propia, G305, 2026-10-07)

Fuentes: lectura de la tabla de features y de `0x8070` con funciones de consulta, y escrituras de prueba en el sector de usuario `0x0001`, cada una autorizada por el usuario y comprobada releyendo el sector. El usuario observó el efecto en el ratón, también después de apagarlo y encenderlo.

### 0x8070 ColorLedEffects (v6, índice 6 en el G305)

- `getInfo` (fn0): `01 00 00 00 14 …`, es decir, 1 zona.
- `getZoneInfo(0)` (fn1): `00 00 01 04 …`: zona 0, ubicación `0x0001` y 4 efectos.
- `getZoneEffectInfo(0, i)` (fn2) da los identificadores `0x0000`, `0x0001`, `0x0003` y `0x000A`. Los campos siguientes (`C005 03E8` y `C105 003C`) no se interpretan.
- Un cambio de luz hecho por otra aplicación (OpenMouse) con el ratón en modo de perfiles integrados **no modifica el perfil** y **se pierde al apagar el ratón**.
- `setZoneEffect` (fn3), verificado en la puerta H2 el 2026-10-07 con el ratón en modo de perfiles integrados:
  - Parámetros: `[zona, posición del efecto en la lista de getZoneEffectInfo, 10 bytes de parámetros, 00]`.
  - Los 10 bytes de parámetros siguen el mismo orden que los bytes `+1..+10` del bloque de luz del perfil: color en `+1..+3`, periodo de la respiración en `+4..+5` y periodo del ciclo en `+6..+7`.
  - Probado con fija roja, respiración roja de 2000 ms y ciclo de 4000 ms. Los tres se vieron al momento, el ratón volvió a la luz de su perfil al apagarlo y encenderlo, y el sector del perfil releído no cambió.
  - El último byte se envió siempre a `00`. Por la documentación pública **se supone** que es el de persistencia; con `00` el cambio no persistió. Nunca se envía otro valor.

### Perfil integrado (formato 3, sector de 255 bytes)

| Bytes | Significado | Evidencia |
| --- | --- | --- |
| `0x03–0x0A` | Ciclo de DPI: 4 valores u16 **little-endian**; `0x0B–0x0C` = `00 00` (fin) | Con las 4 entradas a 1200, el ratón arrancó a 1200. Con 1100/1200/1300/1400, arrancó a 1300 |
| `0x01` | **No** decide el DPI de arranque | Al cambiarlo de `01` a `03`, el arranque siguió en 1300 |
| Posición de arranque del ciclo | **No está en el perfil.** En este ratón siempre fue la tercera entrada, aunque los bytes 1 y 2 valen `01` y `00`; se supone que el ratón la recuerda internamente | Pruebas 6, 8 y 9 |
| `0xDB–0xE5` | **Bloque de luz activo** (11 bytes) | Pruebas 2 a 6 |
| ↳ `+0` | Efecto: `00` apagada, `01` fija, `03` ciclo de colores, `0A` respiración | Observado |
| ↳ `+1..+3` | Color R, G, B (fija y respiración) | Rojo `FF 00 00` y verde `00 FF 00` |
| ↳ `+4..+5` | Periodo de la respiración en ms, big-endian | 2000 y 6000 ms, con ritmo observado de unos 3 y 6 s |
| ↳ `+6..+7` | Periodo del ciclo en ms, big-endian | `1F 40` = 8000 ms, con vueltas observadas de unos 8 s |
| ↳ `+8..+10` | Sin interpretar; se escribieron a `00` sin efecto visible | — |
| `0xD0–0xDA` | Segundo bloque con la misma forma. Cambiarlo a rojo fijo **no tuvo efecto visible** | Prueba 1. Se conserva sin tocar |

- Los cambios del perfil se aplican **al instante**, sin reconectar ni enviar `setCurrentProfile`, y se mantienen al apagar el ratón.
- En el perfil de este usuario, el botón DPI tiene la función `90 0C` y no recorre el ciclo; en la ROM tiene `90 05`. Su significado no está verificado.

## Aviso de conexión del receptor (0x41) (observación propia, G305, 2026-10-07)

- El receptor envía un informe corto HID++ 1.0 (`0x10`) por la colección de página `0xFF00` y uso `0x0001`. Es la misma interfaz que la colección larga que usa la app, pero otra colección.
- Formato: `10 <dispositivo> 41 <byte3> <byte4> …`. Capturado con el G305 en el dispositivo 1:
  - Al apagarlo con el interruptor: `10 01 41 0D 62 74 40`. El bit `0x40` del byte 4 está a 1: enlace caído.
  - Al encenderlo: `10 01 41 0D A2 74 40`. El bit `0x40` está a 0: enlace establecido.
- Justo después del encendido llegan `11 01 04 00 01 01 01 …` (feature `0x1D4B`, en el índice 4) y `11 01 0A 00 01 00 01 …` (feature `0x8090`). No se interpretan.
- Al encenderse, el ratón vuelve al DPI de arranque de su perfil y a la luz guardada en él. Contesta peticiones unos 1,0–1,6 s después del aviso.
- **Uso en la app:**
  - Un hilo por receptor mantiene abierta esa colección **solo para leer**: nunca escribe en ella, y cada handle recibe su propia copia de los informes.
  - Con «enlace establecido» espera 1,5 s, agrupa los avisos repetidos y pide al worker que vuelva a enviar en vivo el DPI y la luz guardados en la app.
  - Verificado con dos ciclos de apagado y encendido: el sensor volvió de 1600 a 900 DPI en 2,2–3,6 s, y la luz se restableció.
- **No verificado:** si el despertar tras reposo emite este aviso.
