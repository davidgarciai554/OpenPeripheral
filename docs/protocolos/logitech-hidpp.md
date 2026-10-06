# Logitech HID++ 2.0

Implementación: `src/protocol/hidpp.rs`, `src/devices/logitech.rs`.

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

Lista de DPI: valores u16 big-endian terminados en 0. Si un valor tiene los tres bits altos a 1 (`0xE000`), indica un paso (`valor & 0x1FFF`) y forma el rango `[mín, paso, máx]`.

## Verificado en hardware (G305)

- Batería 90 %, coherente con G HUB.
- DPI 1000 → 1600 → releído 1600 → restaurado 1000.
- `getOnboardMode` (0x8100 fn2) devuelve `1`, el modo perfiles integrados. En este modo `setReportRate` (0x8060 fn2) se rechaza con error HID++ 2.0. **No se usa:** cambiar a modo host (`setOnboardMode 2`) queda fuera de v0.1 porque no se ha verificado si persiste.
