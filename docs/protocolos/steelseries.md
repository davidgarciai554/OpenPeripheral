# SteelSeries (DPI volátil)

Implementación: `src/protocol/steelseries.rs`, `src/protocol/steelseries_data.rs` y `src/devices/steelseries.rs`.

## Fuente

- rivalcfg (https://github.com/flozz/rivalcfg, commit 779ea52c, WTFPL v2). Los datos de cada modelo se generaron leyendo como texto `rivalcfg/devices/*.py` y `rivalcfg/devices/dpi/truemove_{core,air}.py`, sin ejecutarlos.
- **No verificado en hardware:** no había ningún ratón SteelSeries.

## Qué se usa

- **Escritura:** un *output report* en la interfaz USB de cada modelo, enviado a su colección de fabricante (página ≥ `0xFF00`). La colección exacta en Windows está **sin verificar**. Si la interfaz tiene varias, se usa la primera que aparece en la enumeración.
- **Formato:** `00` (sin id de informe), la orden del modelo, `01` (un preset), el índice del preset elegido (0 o 1 según el modelo) y el código del DPI en little-endian (1 o 2 bytes).
- **Código del DPI:**
  - Por tabla, para los sensores TrueMove Core y Air, en pasos de 100.
  - Lineal, `1 + (dpi − 50) / 50`, para los Prime.
- **No se envía nunca la orden «guardar»** (`0x09`, `0x11` o `0x59`, según el modelo). Por eso el DPI se pierde al desconectar el ratón.
- **Lectura:** el ratón no informa de su DPI. La app muestra «—» hasta que fija uno; el DPI guardado en la app se vuelve a aplicar al conectar.
- **Fuera de alcance:**
  - Modelos de rivalcfg con otras codificaciones (DPI X/Y o «flag») o con otros nombres de ajuste.
  - Los inalámbricos sin DPI por la radio.
