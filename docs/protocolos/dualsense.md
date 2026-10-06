# Sony DualSense (USB)

Implementación: `src/protocol/dualsense.rs`, `src/devices/dualsense.rs`.

## Fuentes

- Estructura de informes del DualSense documentada públicamente, por ejemplo en las estructuras comentadas del controlador del kernel Linux. Se reimplementa sin copiar código.
- Observación propia el 2026-10-06 con un DualSense 054C:0CE6 por USB y Steam en ejecución.

## Batería: input report 0x01 (USB, 64 bytes)

- El mando emite este informe continuamente; se lee el primero con id 0x01.
- Byte 53, contando el id como byte 0:
  - El nibble bajo es el nivel 0–10. Porcentaje = `min(nivel × 10 + 5, 100)`.
  - El nibble alto es el estado: 0 descarga, 1 cargando, 2 completa. Otros valores son errores, y entonces el porcentaje no es fiable.
- Verificado: `0x18` → 85 %, cargando.

## Lightbar: output report 0x02 (USB, 48 bytes)

| Byte (con id) | Valor |
| --- | --- |
| 0 | 0x02 |
| 2 | `valid_flag1` = 0x04 (control de lightbar) |
| 39 | `valid_flag2` = 0x02 (control de *lightbar setup*) |
| 42 | `lightbar_setup` = 0x02 (termina la animación de encendido) |
| 45–47 | R, G, B |

El resto queda a 0 y sin flags, así que no se tocan motores, gatillos, audio ni LEDs de jugador. El color es volátil: se pierde al desconectar.

**Incidencia resuelta:** sin los bytes 39 y 42 el mando ignoraba el color porque mantenía la animación de encendido. Con ellos, el usuario confirmó visualmente el cambio.

## No admitido

- Bluetooth: usa otros ids de informe y requiere CRC32. No está implementado ni verificado.
