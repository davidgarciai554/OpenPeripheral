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
