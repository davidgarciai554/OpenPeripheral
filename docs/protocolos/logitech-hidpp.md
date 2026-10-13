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
| 0x0005 | fn0 longitud, fn1 trozo | Nombre del modelo (se quitan caracteres de control y bidi; máx. 64) | No |
| 0x0005 | fn2 getDeviceType | Tipo (3 ratón, 4 trackpad, 5 trackball). Solo en dispositivos detectados automáticamente: los demás tipos no se abren como ratón | No |
| 0x1000 | fn0 | Batería: %, siguiente nivel, estado (0 descarga; 1, 2, 4 cargando; 3 completa) | No |
| 0x1004 | fn1 | Batería unificada: %, nivel, estado (0 descarga; 1, 2 cargando; 3 completa) | No |
| 0x2201 | fn1 lista, fn2 actual | DPI permitido y actual | No |
| 0x2201 | fn3 setSensorDpi | Cambiar DPI. **Volátil**: no toca los perfiles en flash. | Sí |
| 0x8060 | fn0 lista, fn1 actual | Tasa de sondeo | No |
| 0x8060 | fn2 setReportRate | Cambiar la tasa (intervalo en ms) y releerla. **Volátil**. Solo en ratones sin perfiles integrados o que ya estén en modo host. Nunca se cambia el modo | Sí |
| 0x8100 | fn0 getInfo, fn4 getCurrentProfile, fn5 memoryRead | Leer el perfil integrado activo, el directorio y la copia de fábrica | No |
| 0x8100 | fn6 memoryWriteStart, fn7 memoryWrite, fn8 memoryWriteEnd | Guardar los botones en el perfil de usuario activo. **Persistente**: escribe en la memoria del ratón | Sí |

Lista de DPI: valores u16 big-endian terminados en 0. Si un valor tiene los tres bits altos a 1 (`0xE000`), indica un paso (`valor & 0x1FFF`) y forma el rango `[mín, paso, máx]`.

## Verificado en hardware (G305)

- Batería 90 %, coherente con G HUB.
- DPI 1000 → 1600 → releído 1600 → restaurado 1000.
- `getOnboardMode` (0x8100 fn2) devuelve `1`, el modo perfiles integrados. En este modo `setReportRate` (0x8060 fn2) se rechaza con error HID++ 2.0. **No se usa:** cambiar a modo host (`setOnboardMode 2`) queda fuera de v0.1 porque no se ha verificado si persiste.

## Perfiles integrados (0x8100)

Solo se interpreta lo observado en un G305 (formato de perfil 3) y en un G203 LIGHTSYNC (formato 4, ver más abajo). Cualquier otro formato se trata como «no compatible» y no se lee ni se escribe.

- **getInfo (fn0):** byte 1 formato de perfil, byte 3 número de perfiles, byte 5 número de botones y bytes 7–8 tamaño de sector (u16 big-endian). G305: formato 3, 1 perfil, 6 botones y sectores de 255 bytes.
- **getOnboardMode (fn2):** `1` perfiles integrados, `2` modo host. Antes de leer el perfil se comprueba que sea `1`: en modo host el perfil no está en uso y `getCurrentProfile` devuelve `0x0000`. Si no es `1`, la app muestra «La memoria del ratón está desactivada…» y guarda el DPI en la app.
- **setOnboardMode (fn1), solo con el parámetro `1`:** lo envía el botón «Activar la memoria del ratón», siempre tras una confirmación y solo en modelos verificados. Después la app comprueba que el modo sea `1`. Nunca envía `2`.
  - Verificado en el G203 el 2026-10-09: el ratón siguió en modo `1` tras desconectarlo y reconectarlo, y la memoria del perfil no cambió.
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

### Formato 4: G203 LIGHTSYNC (lectura propia, solo lectura, 2026-10-09)

- Ratón por cable 046D:C092, índice `0xFF`. `getInfo`: `01 04 01 01 01 06 10 00 FF 0A 01`. Formato 4, 1 perfil, 6 botones, 16 sectores de 255 bytes.
- `getOnboardMode` devolvió `2` (modo host). En ese momento estaba en marcha el servicio `logi_lamparray_service` de Logitech, pero no G HUB. **Hipótesis sin verificar:** es ese servicio quien lo pone en modo host.
- Directorio `00 01 01 FF…`, con CRC válido, y perfil de usuario en `0x0001`, también con CRC válido. La copia de fábrica `0x0101` no tiene CRC, como en el G305.
- **Igual que el formato 3:**
  - ciclo de DPI en `0x03`, 4 valores u16 little-endian y `00 00`. Usuario: 800/850/1600/3200; fábrica: 400/800/1600/3200;
  - botones desde `0x20`, 4 bytes cada uno: `80 01` ratón, `80 02` tecla y `90 05` en el botón DPI de fábrica.
- **Distinto o sin interpretar:**
  - Una entrada `00 02 00 00` en el sexto botón del perfil de usuario; se supone una macro en el sector 2, que está vacío. Se conserva sin tocar.
  - La luz: el bloque `0xD0` empieza por `04` y el `0xDB` está a ceros; en la copia de fábrica, `0xDB` usa el efecto `04`, que no está verificado.
  - Nombre del perfil en UTF-16 en `0xA0` («PROFILE_NAME_DEFAULT»).
- **Uso en la app:** en el formato 4 solo se editan botones y DPI. La luz del perfil no se ofrece (`effects` vacío) y «De fábrica» copia DPI y botones, nunca los bloques de luz.
- **Prueba del modo (2026-10-09, con permiso del usuario):**
  - Con la iluminación dinámica de Windows desactivada 15 s en el registro del usuario (y restaurada), el ratón siguió en modo `2`.
  - Se le envió `setOnboardMode(1)` (fn1, parámetro `01`; es la única escritura): respondió `00 00` y pasó a modo `1`. Siguió en `1` durante 30 s y también más de un minuto después, con `getCurrentProfile` = `0x0001`.
  - Conclusión: ningún programa en ejecución lo devolvió al modo host. El modo `2` era un estado que se había quedado puesto, no un control activo. Quién lo dejó así (G HUB en el pasado o el servicio LampArray al conectar) y si se mantiene al desconectarlo **no está verificado**.
- **Escritura desde la app (2026-10-09, el usuario):** tras desconectar y reconectar, el ratón siguió en modo `1`. Se creó la copia original y se guardó desde la app.
  - Relectura de solo lectura del sector `0x0001`: las 4 entradas del ciclo de DPI pasaron a `20 03` (800) y el CRC pasó a `1A 6D`, válido.
  - Los botones y los bloques de luz `0xD0–0xE5` quedaron idénticos a la lectura inicial.
  - El usuario confirma que funciona («ya funciona perfecto»).
- **Botones verificados (2026-10-09):** el lateral trasero pasó de Ctrl+V a Atrás y volvió a Ctrl+V desde la app. La memoria releída cambió solo en esa entrada y en el CRC, y al final quedó idéntica byte a byte. El usuario comprobó físicamente que el lateral pega y el delantero copia.

## Detección automática (2026-10-09)

- **Bluetooth (2026-10-09, plan `cualquier-raton`, fase 3):** un ratón Logitech emparejado por Bluetooth habla HID++ por la colección de página `0xFF43`, en el índice `0xFF` y solo con informes largos. `lookup` acepta esa colección con VID `046D` como `AUTO_LOGITECH_BT`: conexión «Bluetooth», solo ajustes volátiles y el mismo filtro de `getDeviceType`. **No verificado en hardware:** no había ningún ratón Logitech Bluetooth. Lo cubren el simulador y el catálogo (MX Master 3S y 4).

- Cualquier colección `0xFF00/0x0002` con VID `046D` que no esté en `SUPPORTED` se prueba como `AUTO_LOGITECH`: ping (índice `0xFF` por cable, `1–6` tras un receptor), nombre y `getDeviceType`. Solo se aceptan ratones, trackpads y trackballs con nombre.
- Un modelo así no tiene el perfil integrado habilitado (`onboard: false`): `read_profile` responde «no admitido» sin enviar nada. Solo se usan `0x2201` (DPI, volátil), `0x8070` (luz, con el byte de persistencia a 0), la batería y `0x8060` de lectura.
- Las rutas que no responden se omiten en los sondeos periódicos y se reintentan al cambiar los dispositivos, al pulsar «Actualizar», con una notificación de conexión o cada 5 minutos (un ratón dormido tras un receptor no avisa al despertar).
- Guardar el DPI o la luz de un modelo así no escribe en su memoria: se guarda en la configuración de la app, que lo reaplica al conectarlo.
- Probado en un G203 LIGHTSYNC por cable (046D:C092, interfaz 1): responde en `0xFF`, DPI 50–8000 con paso 50 y 1000 Hz.

## Tasa de sondeo editable (2026-10-13, plan `openmouse-deteccion-y-controles`)

- **Fuente:** OpenMouse, `@openmouse/protocol` 0.40.1 (AGPL-3.0-or-later), `dist/drivers/logitech/hidpp.js`: `setLegacyReportRate` (0x8060 fn2 con el intervalo en ms y relectura con fn1) y `ensureHostControl`.
- **Lo que no se porta:**
  - OpenMouse pasa a modo host (`0x8100 setOnboardMode 2`) a los ratones tras un receptor antes de escribir la tasa. Según su propio comentario, es un cambio persistente que deja sin efecto el perfil integrado.
  - Si el ratón por cable rechaza la escritura en vivo, OpenMouse escribe la tasa en el perfil integrado (`writeActiveProfile`, borrado y escritura de flash).
  - OpenPeripheral no hace ninguna de las dos cosas: un rechazo se muestra como error.
- **Cuándo se ofrece:**
  - Solo si el ratón no tiene 0x8100 o ya está en modo host (`getOnboardMode` = 2). El valor tiene que estar en la lista del propio ratón y se relee después de escribirlo.
  - Al principio se ofrecía también por cable, porque OpenMouse intenta la escritura en vivo en esos ratones. Pero el G203 por cable la rechaza en modo `1` (ver «Verificación en hardware»). Desde entonces, en el G305 y el G203 la tasa solo se muestra; la escritura por cable se retiró por decisión del usuario del 2026-10-13.
- **Solo 0x8060:** 0x8061 (tasa extendida, hasta 8000 Hz) confirma el cambio con una notificación asíncrona y no está implementado. Los ratones que solo tienen 0x8061 muestran la tasa sin poder cambiarla.
- **Reaplicar:** la tasa guardada en la app solo se reaplica al reconectar ratones verificados (`SUPPORTED`).
- **Pruebas sin hardware:** `wired_report_rate_is_set_live_and_read_back`, `rejected_report_rate_has_no_fallback_to_the_profile` y `receiver_mouse_in_onboard_mode_never_gets_a_rate` (`src/devices/logitech.rs`). Comprueban que nunca se envía 0x8100 fn1 ni fn6–fn8.
- **Features declaradas** (puerta 1, solo lectura, 0x0001 FeatureSet, 2026-10-13):
  - G203 por cable (`0xFF`): 0000 0001 0003 0005 1801 1802 1806 1E00 1E22 1EB0 2201 18B1 00C2 8060 8071 8100 8110 18A1 8081.
  - G305 tras el receptor (índice 1): 0000 0001 0003 0005 1D4B 1000 8070 8100 8110 8060 8090 00C2 1802 1803 1806 1811 1805 1830 1890 1DF3 1E00 1EB0 1861 18B1 1E22 1801 2201.
  - Ninguno tiene 0x2202 (DPI extendido con lift-off) ni 0x1B04 (reasignación de controles). En estos ratones no hay lift-off ni reasignación de botones fuera del perfil integrado.
- **Verificación en hardware (puertas 2 y 4, G203 por cable, 2026-10-13, autorizadas por el usuario):**
  - Desde la app, con una configuración aislada, se pidió 500 Hz (2 ms) en el G203, que estaba en modo `1`.
  - El ratón respondió con error HID++ 2.0 («función no admitida»). La app mostró el error y la tasa siguió en 1 ms.
  - El sector `0x0001` del perfil, leído antes y después (solo lectura), quedó idéntico byte a byte. El modo siguió en `1` y no se guardó nada en la configuración.
  - Ese ratón no tiene vía volátil para la tasa. La puerta 3 (desenchufar) no hizo falta, porque no cambió nada.
