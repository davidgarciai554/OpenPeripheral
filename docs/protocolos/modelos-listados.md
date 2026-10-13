# Modelos reconocidos por nombre

Implementación: `src/devices/listed_data.rs` y `platform::hid::listed`.

## Qué hace

- Un ratón de esta tabla aparece en «Dispositivos» con su marca y su modelo y el estado «Detectado · sin ajustes».
- **No se abre ni se le envía nada:** solo se usan los datos de la enumeración de Windows. Esto sirve con los receptores que se presentan como «2.4G Mouse» o sin nombre.
- Los ratones que no están en la tabla siguen apareciendo con el nombre que da Windows.

## Fuente

- OpenMouse (<https://github.com/OpenMouse-Project/openmouse>, commit `59b5ac0b`, AGPL-3.0-or-later), en dos archivos:
  - `src/supported-devices-data.ts`: marca, modelo e identificadores de producto.
  - `VENDOR_ID` de `drivers/vendors.js` en `@openmouse/protocol` 0.40.1: identificadores de fabricante.
- Cada identificador de producto se comprobó en el código de ese paquete, leyéndolo sin ejecutarlo (2026-10-13).
- **Quedan fuera** las marcas con varios identificadores de fabricante posibles (Glorious, HyperX, Lamzu, Attack Shark, Ninjutso, MCHOSE, Redragon, WALLHACK, K-snake): no se adivina a cuál pertenece cada modelo. También quedan fuera Logitech, Razer, SteelSeries y ASUS, que tienen driver.
- Contiene 99 modelos de 21 marcas.

## Procedencia y riesgo

- Algunos identificadores vienen de módulos de OpenMouse derivados del software del fabricante: AJAZZ (panel web nacodex), ATK (ATK HUB), Dareu, Finalmouse (xpanel), Incott (IncottHub), Keychron (Launcher) y VAXEE (Control Center).
- Se incluyen por decisión del mantenedor del 2026-10-13 (`CONTRIBUTING.md`, regla 1a). Cada marca está en su propio bloque, así que se puede retirar sin tocar el resto.
- Son nombres e identificadores, no código de protocolo. Esto no es asesoría legal.

## Volatilidad y escrituras

No aplica: estos ratones no reciben ninguna escritura. Según el código de OpenMouse, la mayoría (Pulsar/VGN, Endgame Gear, ATK, Keychron…) solo admite cambios guardados en su memoria. Se habilitarán marca a marca cuando alguien los pruebe con el ratón real.

## Límites conocidos

- La tabla se mantiene a mano. Si pasa de unas 100 entradas o OpenMouse la cambia a menudo, conviene generarla.
- Algunos identificadores de fabricante son genéricos de chips (`0483` STMicro, `04F3` Elan, `093A` Pixart, `1915` Nordic, `04D9` Holtek). Un dispositivo de otra marca con el mismo VID:PID se mostraría con el nombre de la tabla, aunque solo si Windows lo presenta como ratón, teclado o mando.
- **Sin verificar en hardware:** no hay ningún ratón de estas marcas para probarlo.
