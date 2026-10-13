# OpenPeripheral

Control ligero de periféricos para Windows: batería, DPI, luz y botones del ratón, color de teclados y mandos, **sin cuentas y sin telemetría**. Su única conexión a internet, opcional, descarga fotos de los ratones (ver «Seguridad y privacidad»). Es una alternativa mínima a Razer Synapse, Logitech G HUB y similares para lo más habitual.

> *English:* lightweight, account-free peripheral control for Windows (battery, DPI, lighting, mouse buttons). No telemetry; its only network access, optional, downloads mouse photos. Single ~0.9 MB native binary.

## Qué hace

- Muestra la batería de los dispositivos compatibles en la bandeja del sistema, como texto: porcentaje y estado de carga.
- La ventana **Dispositivos** muestra una tarjeta por dispositivo: tipo, conexión y estado, una pila de 5 franjas con el porcentaje (o «Con cable»), DPI, tasa de sondeo, el último color aplicado y la luz guardada del ratón. Sigue el tema claro u oscuro de Windows (o el elegido en «Ajustes»), y el alto contraste.
- **Ajustes del ratón** (botón «Ajustes» de la tarjeta de un ratón compatible), en tres pestañas:
  - **Botones y DPI:** un dibujo esquemático del ratón con un punto por botón. Se elige un botón en el dibujo y su acción en «Asignaciones»: una lista de comandos con buscador (copiar, pegar, pestañas, zoom…), un atajo de teclado propio o un clic (izquierdo, derecho, central, Atrás, Adelante). El botón izquierdo y el botón DPI no se cambian. A su lado, el DPI: el actual, un campo con pasos y valores rápidos. Al guardarlo se escribe en los 4 niveles del botón DPI, porque el ratón no guarda en su perfil con qué nivel arranca.
  - **Iluminación:** apagada, fija, ciclo de colores o respiración, con cualquier color (muestras o valor hexadecimal) y tres velocidades.
  - **Avanzado:** los programas vinculados al perfil activo («Vincular programa…», «Desvincular»).
  - **«Probar»** envía el DPI o la luz al ratón sin guardarlo. **«Guardar en el ratón…»** escribe juntos el DPI, la luz y los botones en la memoria del ratón, con una sola confirmación, y comprueba la escritura releyendo el perfil. Así funcionan aunque cierres la aplicación o uses el ratón en otro PC.
  - **Al iniciarse o al conectar el ratón,** OpenPeripheral vuelve a aplicar en vivo el DPI y la luz guardados, aunque otro programa los haya cambiado. No escribe en la memoria para ello. Si apagas y enciendes el ratón, el receptor lo avisa y los ajustes se reaplican en unos 2–4 s.
  - **Perfiles:** hasta 8 configuraciones con nombre (DPI, luz y botones) guardadas en este PC, como pestañas sobre la página. Pulsar un perfil lo escribe en la memoria del ratón, sin diálogo, y lo comprueba releyendo; si el ratón ya lo tiene, no escribe. «+ Nuevo» crea uno; el menú «Perfil …» renombra, guarda como perfil nuevo, vuelve a aplicar, restaura y elimina. También se cambia de perfil desde el menú del icono de la bandeja («Perfil de G305 ›»), que avisa con un globo si el cambio falla. La primera vez, lo que guarda el ratón se convierte en «Perfil 1».
  - **Perfiles por juego:** con OpenPeripheral abierto, al abrir un juego de Steam, Xbox, Epic o GOG que no conoce, Windows muestra un aviso «Juego nuevo» con «Configurar… / Ahora no / No preguntar más». «Configurar…» crea un perfil con el nombre del juego y lo vincula. Después, al abrir ese juego se usa su perfil y al cerrarlo vuelve el anterior: el DPI y la luz se aplican al momento sin escribir en el ratón, y los botones solo se escriben en su memoria si cambian. Cualquier otro programa se vincula desde «Avanzado».
  - **Restaurar** (menú «Perfil …»): «Restaurar perfil original del ratón…» vuelve a la copia tomada antes del primer cambio; «Valores de fábrica…» devuelve el DPI, la luz y los botones de fábrica; «Restaurar originales» deja de aplicar los ajustes de la app.
- **Ajustes** (botón de la cabecera de Dispositivos, o «Ajustes…» en la bandeja): iniciar con Windows; tema Sistema, Oscuro o Claro; cambio automático de perfil y aviso de juegos nuevos, para todos los ratones; juegos sin responder y juegos por los que no se pregunta; notificaciones de Windows y aviso de programas en conflicto.
- El color de teclados y mandos, y el DPI de otros dispositivos, se cambian desde la línea de órdenes; sus pantallas de ajustes están pendientes.
- Avisa si hay otro software de periféricos en ejecución que pueda sobrescribir los ajustes, como G HUB, Synapse, Steam Input u OpenLogi. Nunca lo cierra.

## Dispositivos verificados

Solo se listan dispositivos probados en hardware real. Además:

- **Otros ratones Logitech HID++ 2.0** (por cable, con receptor o por Bluetooth; Bluetooth sin probar con un ratón real) se detectan automáticamente y se marcan «Detectado automáticamente». Ofrecen lo que declare el propio ratón (DPI, luz, batería y tasa de sondeo de lectura), siempre como ajuste volátil. Sus botones y su perfil integrado no se leen ni se escriben hasta verificar el modelo (ver [CONTRIBUTING.md](CONTRIBUTING.md)). Probado con un G203 LIGHTSYNC por cable (046D:C092): DPI leído, cambiado, releído y restaurado.
- **Ratones Razer, SteelSeries y ASUS** (plan `Planes/2026-10-09-cualquier-raton.md`): se detectan automáticamente y solo se les cambia el DPI **sin guardarlo en el ratón**. Se pierde al desconectarlo; OpenPeripheral lo vuelve a aplicar al conectarlo si lo guardas en la app. **Ninguno se ha probado con un ratón real**: el comportamiento sale de hechos públicos de los protocolos y de un banco de ratones virtuales (`src/devices/sim.rs`, `src/devices/catalog.rs`).
  - **Razer:** cualquier ratón que responda, solo con lecturas, a la versión de firmware y al DPI (identificadores de transacción `0x1F`, `0x3F` y `0xFF`). DPI de 100 a 16000 en pasos de 50, porque el protocolo no dice el rango del sensor.
  - **SteelSeries:** 21 modelos con su orden de DPI conocida (Rival 3 y 5, Aerox 3, 5 y 9, Prime y variantes), con datos de rivalcfg. El ratón no informa de su DPI: la página muestra «—» hasta que OpenPeripheral fija uno.
  - **ASUS ROG y TUF:** 24 modelos con datos de libratbag. Se cambia el DPI del perfil de DPI activo. Quedan fuera los modelos tras el receptor Omni y los de DPI X/Y separados.
- **Para cualquier ratón:** «Ajustes» › «General» › «Velocidad del puntero de Windows». Vale para todos los ratones a la vez y no es DPI.
- **Ratones, teclados y mandos de otras marcas** aparecen con «Detectado · sin ajustes». Solo se usan los datos que da Windows (nombre y tipo): nunca se abren ni se les envía nada.

| Dispositivo | Conexión | Batería | DPI | Color | Notas |
| --- | --- | --- | --- | --- | --- |
| Logitech G305 Lightspeed | Receptor LIGHTSPEED (046D:C53F) | Sí | Sí | Luz (4 efectos) | DPI, luz y botones en la memoria del ratón; tasa de sondeo solo de lectura (ver abajo) |
| Logitech G203 LIGHTSYNC | USB (046D:C092) | No (cable) | Sí | No (declara 0x8071, no implementado) | Botones y DPI en la memoria del ratón (perfil de formato 4; escritura y restauración de botones y DPI verificadas en hardware el 2026-10-09). Si G HUB u otro programa le desactivó la memoria (modo host), «Activar la memoria del ratón» la vuelve a activar tras confirmarlo |
| Razer BlackWidow V3 Tenkeyless | USB (1532:0A24) | — | — | Estático | Synapse puede sobrescribir el color |
| Sony DualSense | USB (054C:0CE6) | Sí | — | Lightbar | Bluetooth no admitido todavía |

**Pendiente de verificar:** Razer BlackShark V2 Pro (2020) y la batería de dispositivos Bluetooth.

## Uso

Descarga `openperipheral.exe` de la página de *Releases*, comprueba su suma SHA-256 y ejecútalo. No necesita instalación ni permisos de administrador. Windows SmartScreen puede avisar porque el binario no está firmado.

- **Clic o Enter** sobre el icono de la bandeja: abre la ventana Dispositivos. En ella, Tab recorre los botones y Esc la cierra.
- **Clic derecho o Mayús+F10**: menú con batería, «Abrir OpenPeripheral», «Actualizar ahora», «Ajustes…», «Iniciar con Windows» y «Salir».
- Con el teclado, **Win+B** lleva el foco a la bandeja.

Línea de órdenes (diagnóstico y ajustes; no guarda nada):

```text
openperipheral --list
openperipheral --apply 1 dpi=1600
openperipheral --apply 0 color=#FF8000
```

La configuración se guarda en `%APPDATA%\OpenPeripheral\config.toml`. Identifica cada dispositivo solo por `VID:PID/modelo`, sin números de serie. Antes de cambiar por primera vez los botones de un ratón, se guarda una copia de su perfil en `%APPDATA%\OpenPeripheral\backups\`. Esa copia nunca se sobrescribe y tampoco lleva número de serie.

## Limitaciones conocidas

- **Solo Windows 10/11.**
- **Tasa de sondeo del G305:** en modo «perfiles integrados» el ratón rechaza el cambio. Cambiar a modo host no está verificado como no persistente, así que en esta versión solo se muestra.
- Si otro programa controla el mismo dispositivo, puede deshacer los cambios. Esto incluye las páginas web que usan WebHID, como OpenMouse: OpenPeripheral no puede detectarlas.
- Dos dispositivos del mismo modelo conectados a la vez se muestran como uno y comparten la copia del perfil original. No cambies los botones de dos ratones iguales desde el mismo PC: «Restaurar perfil original» escribiría la copia del primero.
- **Perfiles:** se guardan por modelo (`VID:PID/modelo`), así que dos ratones del mismo modelo comparten la lista y el perfil activo; cambiar de perfil solo escribe en el ratón conectado. Un perfil creado a partir de un ciclo de DPI con niveles distintos no tiene DPI propio: al usarlo, el DPI del ratón se queda como esté. Cambiar de perfil tarda unos 2–3 s y no funciona con G HUB abierto.
- **Perfiles por juego:**
  - Solo funcionan con OpenPeripheral abierto. Un programa se detecta cuando pasa a primer plano: uno que ya estaba abierto al arrancar la app, o que nunca toma el primer plano, no se detecta. Los programas que se ejecutan como administrador se ignoran.
  - Las asociaciones guardan la ruta completa del ejecutable en `config.toml`: si mueves el juego de carpeta, hay que volver a asociarlo. Las bibliotecas se leen al arrancar la app: una biblioteca nueva necesita reiniciarla. Epic y GOG no se han probado con instalaciones reales.
  - El DPI y la luz aplicados en vivo durante la partida se pierden si apagas el ratón. Un clic en el aviso de Windows cuando la app ya está cerrada no hace nada.
  - Para mostrar los avisos, la app registra su nombre en `HKCU\Software\Classes\AppUserModelId\OpenPeripheral` con el primer aviso. Se quita con «Quitar avisos de Windows» (en «Ajustes») o borrando esa clave.
  - No hay límite de escrituras automáticas en la memoria del ratón: solo se escriben los botones cuando cambian, 5 s después de abrir el programa.
- «Ajustes» solo tiene pantalla para ratones. En teclados y mandos aparece como «próximamente», y su color se cambia con `--apply`.
- **Guardar en el ratón (DPI, luz y botones):**
  - Solo en ratones con perfiles integrados de formato verificado (por ahora, el G305 y el G203 LIGHTSYNC).
  - El DPI guardado ocupa los 4 niveles del botón DPI. Los niveles distintos se recuperan con «Restaurar perfil original…» o «Valores de fábrica…».
  - Doble clic, multimedia y «desactivar» no se ofrecen todavía: su formato no está comprobado. Los atajos con la tecla Windows (Win + D, Win + E…) se eligen en el grupo «Windows» de «Comandos»; en «Teclas» no se pueden escribir, porque Windows actúa al pulsar esa tecla.
  - El dibujo del ratón es esquemático y propio: el G305 tiene el suyo; los demás ratones usan una silueta genérica.
  - El guardado se bloquea mientras G HUB está abierto, porque podría reescribir el perfil.
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

- **Única conexión a internet: fotos de los ratones**, desactivable en «Ajustes» › «Descargar fotos de los ratones» (activada por defecto).
  - Pide por HTTPS a `img.openmouse.app` (el servidor de imágenes del proyecto OpenMouse, servido por Cloudflare) la foto de los modelos que la app conoce. Hoy son las del G305/G304 y el G203. El servidor ve tu IP y qué foto pides. No se envía nada más: ni cookies ni identificadores.
  - Solo se pide una lista fija de archivos, nunca con el nombre del ratón. Una foto solo se usa si su huella SHA-256 coincide con la que lleva la app. Si no coincide, si no hay conexión o si falla algo, se ve el dibujo de la app.
  - Las fotos se guardan en `%APPDATA%\OpenPeripheral\images\`. Se borran al desactivar la opción y cuando la app lleva 14 días sin ver un ratón de ese modelo.
  - Las fotos son de sus fabricantes y sus derechos no están verificados. No están en este repositorio ni en las versiones publicadas.
- Fuera de eso, la aplicación no envía nada a ningún sitio.
- Solo abre interfaces HID de una lista blanca y la colección HID++ de los dispositivos Logitech (para detectar otros modelos), y solo puede enviar comandos tipados y documentados. A un modelo Logitech no verificado solo le envía consultas y ajustes volátiles (DPI y luz). Los dispositivos de otras marcas que no están en la lista no se abren. No hay forma de mandar bytes arbitrarios, y no existe código para firmware ni DFU.
- La única escritura en la memoria de un dispositivo es el perfil integrado del ratón: DPI, luz y botones ([detalles](docs/protocolos/logitech-hidpp.md#perfiles-integrados-0x8100)):
  - va solo al sector del perfil de usuario activo, nunca al directorio ni a la copia de fábrica;
  - solo cambia los bytes del ciclo de DPI, del bloque de luz activo y de los botones editados, además del CRC;
  - requiere confirmación;
  - se verifica releyendo y, si falla, deja el perfil como estaba.
- Funciona como usuario normal (manifiesto `asInvoker`).

## Licencia

Copyright (C) 2026 Chidev.

OpenPeripheral es software libre: puedes redistribuirlo y modificarlo según los términos de la [GNU Affero General Public License](LICENSE), versión 3 o (a tu elección) cualquier versión posterior, publicada por la Free Software Foundation (`AGPL-3.0-or-later`). Se distribuye sin ninguna garantía; consulta la licencia para más detalles.

Código fuente: <https://github.com/davidgarciai554/OpenPeripheral>

Datos y código de terceros con licencias compatibles:

- **libratbag** (MIT, © 2021 Kyoken y colaboradores de libratbag): la orden de DPI de los ratones ASUS (`src/protocol/asus.rs`, que conserva su aviso) y los datos de sus modelos (`src/protocol/asus_data.rs`).
- **rivalcfg** (WTFPL v2, © Fabien Loison y colaboradores): los datos de DPI de los ratones SteelSeries (`src/protocol/steelseries_data.rs`). Logitech, Razer, Sony y sus productos son marcas de sus respectivos propietarios; este proyecto no está afiliado a ellos.
