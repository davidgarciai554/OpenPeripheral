# Razer (informe de 90 bytes)

Implementación: `src/protocol/razer.rs`, `src/devices/razer.rs`.

## Fuentes

- Descripciones públicas del formato de informe de Razer, como la documentación de la comunidad sobre el protocolo USB. Se reimplementa sin copiar código: OpenRazer es GPL-2.0.
- Observación propia el 2026-10-06 con un BlackWidow V3 Tenkeyless 1532:0A24, firmware v1.2.

## Transporte

- *Feature report* con id 0. Se usa la interfaz 2 (página `0x0001`, uso `0x0002`), la única que respondió en la sonda.
- Tras enviar se espera unos 25 ms antes de leer la respuesta. Si el estado es `0x01` (ocupado), se reintenta hasta 4 veces.

| Byte | Campo |
| --- | --- |
| 0 | Estado (petición 0x00; respuesta 0x02 éxito, 0x01 ocupado, 0x03 fallo, 0x04 timeout, 0x05 no admitido) |
| 1 | Id de transacción: `0x1F` (verificado) |
| 2–3 | Paquetes restantes |
| 4 | Tipo de protocolo (0) |
| 5 | Tamaño de datos |
| 6 | Clase de comando |
| 7 | Id de comando |
| 8–87 | Argumentos |
| 88 | CRC: XOR de los bytes 2 a 87 |
| 89 | Reservado |

La respuesta se acepta solo si coinciden clase e id, el estado es éxito y el CRC es correcto.

## Comandos usados

| Clase | Id | Args | Uso | Escribe |
| --- | --- | --- | --- | --- |
| 0x00 | 0x81 | 2 | Versión de firmware; también sirve para saber si responde | No |
| 0x0F | 0x02 | `00 05 01 00 00 01 R G B` | Efecto estático en la retroiluminación (LED 0x05), **varstore 0x00 = no se guarda** | Sí |

## Verificado en hardware

- El firmware responde con tid 0x1F en la interfaz 2.
- Color estático rojo y verde: el usuario lo confirmó visualmente. Razer Synapse estaba instalado, pero no en ejecución.

## Ratones (DPI volátil, 2026-10-09, plan `cualquier-raton`)

- **Hechos de protocolo** (consultados en descripciones públicas; OpenRazer es GPL y no se copia código):
  - Leer DPI: clase `0x04`, id `0x85`, 7 bytes. La respuesta trae almacenamiento, X (u16 big-endian) e Y.
  - Fijar DPI: clase `0x04`, id `0x05`, 7 bytes: `[almacenamiento, X alto, X bajo, Y alto, Y bajo, 0, 0]`.
  - El id de transacción depende de la generación del ratón: `0x1F` en los recientes, `0x3F` en la familia DeathAdder V2 y `0xFF` en los antiguos.
- **Uso en la app:**
  - **Detección:** se prueba cada colección de ratón o de teclado de un VID `1532` sin entrada propia, con dos lecturas: firmware y DPI, con cada id de transacción. Si ninguna responde con un DPI válido, no es un ratón; es lo que pasa con teclados y auriculares.
  - **Escritura:** almacenamiento `0x00` (no se guarda). Si el ratón lo rechaza, falla sin probar el modo persistente.
  - **Rango:** el protocolo no da el del sensor, así que se ofrece 100–16000 en pasos de 50. Si el ratón informa de un DPI mayor, el rango sube hasta ese valor.
  - **Comprobación:** tras fijar el DPI se vuelve a leer; si no coincide, se informa de error. Que el modo «no guardar» (`0x00`) funcione en un ratón real no está verificado.
  - **Nombre:** el que da Windows.
- **No verificado en hardware:** no había ningún ratón Razer. Lo cubren el simulador (`sim::razer`) y el catálogo.
