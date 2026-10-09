# ASUS ROG / TUF (DPI volátil)

Implementación: `src/protocol/asus.rs` (incluye el aviso MIT de libratbag), `src/protocol/asus_data.rs` y `src/devices/asus.rs`.

## Fuente

- libratbag (https://github.com/libratbag/libratbag, commit 8235b5bb, MIT, © 2021 Kyoken): `src/asus.c`, `src/asus.h` y `data/devices/asus-*.device`, leídos como texto.
- **No verificado en hardware:** no había ningún ratón ASUS.

## Qué se usa

- **Informes:** de 64 bytes, sin id de informe (con hidapi se antepone `00`). Orden de 2 bytes en little-endian y luego los parámetros. La respuesta es el siguiente informe de entrada; antes de cada petición se descartan los informes pendientes. El código `0xAAFF` indica error, ratón dormido o desconectado.
- **Lecturas** (sirven también para detectarlo):
  - `0x0012` (perfil): el preset de DPI activo es el byte 11 menos 1.
  - `0x0412` (ajustes): el DPI del preset `i` está en los bytes `4 + 2i`, little-endian, y vale `código × 50 + 50`. Con la peculiaridad `DOUBLE_DPI`, el doble.
- **Escritura:** `0x3151` con los parámetros preset, `0` y el código del DPI (`(dpi [/2 con DOUBLE_DPI] − 50) / 50`, little-endian). Solo cambia el preset activo.
- **No se envía nunca «guardar»** (`0x0350`). Por eso el DPI se pierde al desconectar el ratón.
- **Fuera de alcance:** el receptor Omni (id de informe `0x03`) y los modelos con DPI X/Y separados.
