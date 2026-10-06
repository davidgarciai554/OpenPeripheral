# Diseño: OpenPeripheral — vista «Dispositivos» y ajustes del ratón

> Estado: aprobado (diseño). La vista «Dispositivos» está implementada. La pantalla «Ajustes del ratón» (aprobada el 2026-10-06) requiere un plan propio aprobado.
>
> Este documento es la fuente de verdad de diseño de la interfaz de OpenPeripheral. Debe leerse antes de modificarla. **Sustituye** la dirección anterior («controles clásicos de Windows, sin identidad propia»), aprobada en el plan v0.1, que no admitía tema oscuro.

## Propósito y alcance

- **Producto o área:** ventana principal de OpenPeripheral, una aplicación de escritorio para Windows 10/11.
- **Objetivo de la interfaz:** ver de un vistazo qué periféricos hay, si responden, su batería y lo que se puede ajustar en cada uno.
- **Persona, contexto y plataforma prioritaria:** quien usa periféricos de varias marcas en su PC con Windows y quiere evitar el software del fabricante. Suele tener de 1 a 6 dispositivos.
- **Trabajo principal:** comprobar la batería y el estado, y entrar a los ajustes de un dispositivo.
- **Incluido:**
  - Vista «Dispositivos» con encabezado, aviso de software en conflicto, rejilla de tarjetas, estados, tema oscuro y claro, y ventana estrecha.
  - Pantalla **«Ajustes del ratón»**: DPI, botones, tasa de sondeo y restauración. Su estructura (cabecera común y secciones) será la base de teclados, cascos y mandos.
- **Excluido (por ahora):**
  - Pantallas de ajustes de teclados, cascos y mandos. Su botón «Ajustes» sigue siendo un marcador sin acción (desviación 2).
  - Macros, perfiles múltiples y el ciclo de DPI del botón DPI.
  - Navegación lateral.
  - Imágenes de producto.
  - Animaciones.
- **Señal observable de éxito:** con la ventana abierta, en menos de 2 segundos se sabe qué dispositivo tiene poca batería y cuál no responde, sin depender del color.

## Entrada y referencias

### Petición original

- Resumen fiel: interfaz visual para ver los dispositivos conectados, al estilo de OpenMouse y OpenLogi. Tema «negro y claro», sin nada visual añadido. La batería se muestra como una pila de colores con 5 franjas y el porcentaje al lado. Icono propio para dispositivos con cable, centrado. Icono de mando completo.

### Referencias examinadas

| Fuente | Tipo | Observado | Inferido | Decisión adoptada o descartada | Limitación |
| --- | --- | --- | --- | --- | --- |
| OpenMouse (captura del usuario) | Captura | Barra lateral de 7 secciones; tarjetas grandes con foto del ratón; nombre, Hz y batería como iconos; aviso flotante «Conectado a…» | Diseño centrado en pocos ratones; la foto da identidad | **Adoptado:** tarjetas con nombre y pocos metadatos. **Descartado:** foto de producto (sin derechos), barra lateral (hay una sola vista) y aviso flotante (el estado debe quedar visible) | Solo una captura estática |
| OpenLogi (captura del usuario) | Captura | Lista de filas: nombre, chip de tipo, «● Conectado · Receptor · Canal», batería a la derecha, menú «…», contador «3 dispositivos», botón azul «Añadir dispositivo» | Los desconectados se muestran atenuados, no se ocultan | **Adoptado:** contador, línea de estado escrita, desconectados visibles y atenuados. **Descartado:** acento azul y selectores de vista | Solo una captura estática |

### Exploración visual

| Artifact, enlace o versión | Dirección evaluada | Decisión extraída | Estado |
| --- | --- | --- | --- |
| https://claude.ai/artifact/M4WBLhzC2f7Q8eaHis2Qub (v11) | Tarjetas en tema oscuro y claro, estados, ventana de 420 px | Tarjetas, pila de 5 franjas con color, icono de cable, icono de mando completo | Aprobado el 2026-10-06 |
| Mismo lienzo (v17): mesas «Ajustes del ratón», «Botones — edición», estados del DPI, estados del guardado de botones y ventana de 420 px | Misma ventana con «← Dispositivos»; DPI con campo, −/+ y valores rápidos; botones guardados en la memoria del ratón | Todo lo de la sección «Pantalla Ajustes del ratón» | Aprobado el 2026-10-06 |

El enlace es solo una referencia. Este documento es la fuente de verdad.

## Dirección de diseño aprobada

- **Idea rectora:** instrumental y sobrio. Negro o claro, tipografía y espacio. El único color con significado es el **nivel de batería** y, como dato, el color que el usuario ha aplicado.
- **Personalidad y tono:** una herramienta discreta y precisa. Textos cortos en español, descriptivos, sin jerga.
- **Densidad de información:** media. Una tarjeta por dispositivo con 5 bloques: tipo, identidad, energía, capacidades y acción.
- **Principio de composición:** rejilla de tarjetas centrada con ancho máximo. El orden de lectura es de arriba abajo dentro de cada tarjeta.
- **Dirección que se evita:** acentos de marca, gradientes, sombras, cristal o desenfoque, fotos de producto, avisos flotantes que desaparecen y métricas decorativas.
- **Riesgo visual deliberado y motivo:** colores semánticos en la pila (verde, ámbar, rojo) dentro de una interfaz monocroma. Lo pidió el usuario y hace que la batería baja salte a la vista. El color nunca va solo: lo acompañan el número de franjas, el porcentaje y el texto «Batería baja».

## Arquitectura e interacción

- **Navegación y orientación:** una sola ventana. «Ajustes» en la tarjeta de un ratón sustituye la vista «Dispositivos» por «Ajustes del ratón», **en la misma ventana**. El botón «‹ Dispositivos» (y Esc) vuelve, con el foco devuelto al «Ajustes» de esa tarjeta. La bandeja del sistema sigue siendo el acceso principal.
- **Jerarquía de pantallas y contenidos:**
  1. Encabezado: «Dispositivos», contador «N conectados» y el botón «Actualizar».
  2. Aviso de software en conflicto (solo si lo hay).
  3. Rejilla de tarjetas.
- **Flujo principal:** abrir la ventana desde la bandeja, leer el estado y pulsar «Ajustes» en la tarjeta.
- **Acciones primarias y secundarias:** «Ajustes» en cada tarjeta es la primaria; «Actualizar» en el encabezado es secundaria. Ambas son botones con texto.
- **Patrones seleccionados:**
  - Tablero de tarjetas para una colección pequeña.
  - Vacío orientado a acción.
  - Error recuperable.
  - Carga con forma final.
- **Patrones descartados y motivo:**
  - App shell con barra lateral: no hay secciones reales.
  - Lista de filas: el usuario prefirió tarjetas.
  - Avisos flotantes: los estados deben persistir.

### Tarjeta de dispositivo (orden fijo)

1. Icono del tipo (32 px; 28 px en ventana estrecha) y el tipo en texto: «Ratón», «Teclado», «Mando».
2. Nombre (máximo 2 líneas, luego «…» con el nombre completo accesible) y línea de estado: «Conectado · Receptor LIGHTSPEED», «Conectado · USB», «Sin respuesta · ¿apagado o en reposo?» o «Desconectado».
3. Energía, que puede ser:
   - **Pila de 5 franjas**, el porcentaje (22 px, seminegrita) y el estado de carga («descargando», «cargando», «carga completa»). Con 0–20 % añade «Batería baja» en seminegrita.
   - **Con cable:** icono de enchufe, «Con cable» y «sin batería».
   - **Sin dato:** se omite el bloque.
4. Capacidades en texto: «1000 DPI · 1000 Hz», o «Color aplicado» con una muestra de 14 px y el valor hex. Es siempre el **último color aplicado por la app**, porque el dispositivo no permite leer el suyo.
5. Botón «Ajustes», alineado a la derecha, con nombre accesible «Ajustes de <dispositivo>».

### Pila de batería

- **Forma:** contorno de 40×18 con borne a la derecha y 5 franjas internas iguales.
- **Franjas encendidas** (una cada 20 %, redondeando hacia arriba):

  | Carga | Franjas | Color |
  | --- | --- | --- |
  | 0–20 % | 1 | `battery.low` |
  | 21–40 % | 2 | `battery.mid` |
  | 41–60 % | 3 | `battery.high` |
  | 61–80 % | 4 | `battery.high` |
  | 81–100 % | 5 | `battery.high` |

- **Franjas apagadas:** `battery.empty`.
- **Sin respuesta:** pila en `battery.stale` con contorno discontinuo y el texto «último valor conocido».
- **Error de batería** del dispositivo: se sustituye por el texto «Error de batería», sin pila.
- **Nombre accesible:** «Nivel de batería: N de 5». El porcentaje y el estado se leen como texto.

### Iconografía

Pictogramas propios de trazo (1,6 px; 1,5 px en pila y cable), monocromos en `text.primary` o `battery.outline`, decorativos (`aria-hidden`) cuando hay texto al lado:

- **Ratón:** cuerpo redondeado con línea central y separación de botones.
- **Teclado:** rectángulo con teclas y barra espaciadora.
- **Mando:** contorno con asas, cruceta a la izquierda y 4 botones en rombo a la derecha.
- **Cable:** onda que entra **por el centro** del enchufe, con dos patillas. Ocupa el mismo espacio que la pila.
- **Aviso:** círculo con «!». **Error:** triángulo con «!».
- **Volver:** chevrón a la izquierda («‹»), dentro del botón «Dispositivos». **Correcto:** marca de verificación junto a los mensajes de éxito. Siempre van acompañados de texto.

### Pantalla «Ajustes del ratón»

- **Composición:** una columna centrada de **760 px como máximo** (más legible que 1080 para formularios), con márgenes de 32 px (16 px en estrecha) y 20 px entre bloques. Cada sección es una superficie `surface.card` con borde `border.card`, radio 10 y relleno de 20 px (16 px en estrecha).
- **Orden:**
  1. Botón «‹ Dispositivos».
  2. **Cabecera:** icono de 40 px (32 px en estrecha), nombre (26 px seminegrita; 22 px en estrecha), «Ratón · Conectado · Receptor LIGHTSPEED» y, a la derecha, la pila con el porcentaje y el estado. En estrecha, la pila baja a su propia línea.
  3. Aviso de software en conflicto (solo si lo hay; p. ej. «Puede sobrescribir el DPI»).
  4. **Sensibilidad (DPI).**
  5. **Botones.**
  6. **Tasa de sondeo.**
  7. **Restaurar.**
  8. Nota final en `text.tertiary`: «Los cambios no se guardan en la memoria del ratón: OpenPeripheral los vuelve a aplicar al reconectarlo» (se refiere al DPI; los botones sí se guardan en el ratón y su sección lo dice).
- **Énfasis de la acción principal** de cada sección («Aplicar», «Guardar en el ratón…», «Reintentar»): borde `text.primary` y texto seminegrita, sin color de acento. Las demás acciones usan el botón con contorno estándar.

#### Sensibilidad (DPI)

- Título, y una descripción con el rango leído del ratón: «Admite de 200 a 12000, en pasos de 50».
- **Nuevo valor:** etiqueta, botón «−», campo numérico (96 px; ocupa el ancho en estrecha) y botón «+». Los botones suman o restan un paso (50) y se deshabilitan en el mínimo o el máximo, con nombre accesible «Subir 50 DPI (máximo alcanzado)». El campo tiene fondo `bg.app` y borde `border.control`.
- **Aplicar** al lado del campo y **«Actual: N DPI»** en texto. El DPI **solo se envía al pulsar «Aplicar»** (o Enter en el campo).
- **Valores rápidos:** 400 · 800 · 1600 · 3200 (solo los que estén dentro del rango del ratón). Rellenan el campo, no aplican. El elegido se marca con fondo `surface.selected`, borde `text.primary`, seminegrita y estado presionado, nunca solo con color. En estrecha se muestran en una rejilla de 4.
- **Línea de estado** de la sección:
  - Sin aplicar: «Pulsa “Aplicar” para enviar 1600 DPI al ratón».
  - Aplicando: «Aplicando 1600 DPI…», con los controles deshabilitados.
  - Éxito: marca de verificación y «DPI 1600 aplicado y guardado. Se volverá a aplicar al reconectar el ratón».
  - Valor no válido: icono de error y «**Error:** 1625 no es válido. Usa un valor de 200 a 12000 en pasos de 50 (por ejemplo, 1600 o 1650)». El campo lleva borde de 2 px y estado inválido, y el foco vuelve a él.
  - Sin respuesta: «**Error:** el ratón no respondió; sigue en 1000 DPI. Muévelo para despertarlo y vuelve a intentarlo», con «Reintentar».

#### Botones

- Descripción: «Se guardan en la memoria del ratón: funcionan aunque cierres OpenPeripheral y en otros PC».
- **Tabla** de botones: nombre (Izquierdo, Derecho, Central (rueda), Lateral trasero, Lateral delantero, Botón DPI), acción actual y «Cambiar». La lista sale del perfil del ratón; el G305 declara 6 botones.
  - **Izquierdo:** «No se cambia», sin botón. Nunca se puede reasignar ni desactivar.
  - **Botón DPI:** muestra «Ciclo de DPI del perfil» mientras no se conozca su ciclo real.
- **Edición** («Cambiar»): la fila se expande con fondo `surface.selected`, contorno `border.control` y radio 8.
  - Selector nativo **«Acción»** con tres grupos:
    - Clics y navegación: Clic izquierdo, Clic derecho, Clic central, Atrás, Adelante, Doble clic.
    - Multimedia: Reproducir / pausa, Siguiente, Anterior, Subir volumen, Bajar volumen, Silenciar.
    - Otras: Atajo de teclado…, Desactivar.
  - Con **«Atajo de teclado…»** aparece un campo de captura (de solo lectura, que muestra «Ctrl + C») con «Borrar». Ayuda: «Con el campo seleccionado, pulsa la combinación. Tab y Esc no se capturan: sirven para salir del campo».
  - «Aceptar» y «Cancelar» cierran la edición de la fila.
- **Cambio pendiente:** la fila muestra la acción nueva en seminegrita y la etiqueta con contorno «Cambiado · antes: Adelante».
- **Barra de pendientes**, al pie de la sección y solo si hay cambios: «**N cambios sin guardar.** Todavía no se han escrito en el ratón», con «Descartar» y «Guardar en el ratón…». Al salir con cambios pendientes se pregunta si se descartan.
- **Confirmación** (diálogo modal sobre `overlay.scrim`):
  - Título «¿Guardar N cambios en la memoria del ratón?», con la lista de cambios.
  - Texto: «El ratón los conservará aunque cierres OpenPeripheral o lo uses en otro PC. Antes se guardará una copia del perfil actual para poder restaurarlo».
  - Botones «Cancelar» (con el foco inicial) y «Guardar en el ratón». Esc cancela; al cerrar, el foco vuelve a «Guardar en el ratón…».
- **Guardando:** «Guardando en el ratón… No apagues ni desconectes el ratón hasta que termine». La sección se bloquea.
- **Guardado:** marca de verificación y «Botones guardados en el ratón. Se comprobó la escritura leyendo de nuevo el perfil».
- **Error:** panel de alerta con «**Error:** no se pudieron guardar los botones», la causa, «Tus cambios siguen pendientes en esta pantalla», y los botones «Reintentar» y «Restaurar perfil original…».
- **Copia y restauración:** «Copia del perfil original guardada en este PC el <fecha>» y «Restaurar perfil original…», con su propia confirmación («Se escribirá en el ratón la copia guardada el <fecha>. Se perderán los botones que hayas cambiado»; Cancelar / Restaurar).
- **Ratón no compatible**, o protocolo no verificado: borde discontinuo y «Este ratón todavía no permite cambiar sus botones desde OpenPeripheral». No se muestra ningún control que no funcione.

#### Tasa de sondeo y Restaurar

- **Tasa de sondeo:** «1000 Hz · solo lectura» y «No se puede cambiar mientras el ratón usa sus perfiles integrados».
- **Restaurar:** «Vuelve al DPI que tenía el ratón al detectarlo (N) y olvida los ajustes guardados» y «Restaurar originales» (sin confirmación: es volátil y no toca los botones).

#### Ratón desconectado con la pantalla abierta

La cabecera pasa a «Desconectado» en `text.tertiary` y todos los controles se deshabilitan. Aparece el texto «Conecta o enciende el ratón para cambiar sus ajustes. Los controles se activarán solos cuando vuelva». No se cierra la pantalla, y los cambios de botones sin guardar se conservan.

## Sistema visual

### Color y superficies

- **Superficie y fondo:** fondo `bg.app` y tarjetas `surface.card` con borde `border.card` de 1 px, sin sombras.
- **Texto y jerarquía:**
  - `text.primary` para títulos, porcentaje y nombres.
  - `text.secondary` para metadatos.
  - `text.tertiary` para etiquetas de sección y desconectados.
- **Acción y énfasis:** sin color de acento. Los botones llevan contorno `border.control` y texto `text.primary`. El énfasis se marca con peso tipográfico.
- **Estados semánticos:** solo la pila usa color (`battery.*`). Los avisos y errores son monocromos, con icono y texto («Error: …»).
- **Contraste objetivo:** texto ≥ 4,5:1 y gráficos con significado ≥ 3:1, medidos con la fórmula WCAG 2.x el 2026-10-06:

  | Par | Oscuro | Claro |
  | --- | --- | --- |
  | Texto primario sobre tarjeta | 16,15:1 | 18,86:1 |
  | Texto secundario sobre tarjeta | 7,03:1 | 6,63:1 |
  | Texto secundario sobre fondo | 7,65:1 | 6,03:1 |
  | Texto terciario / desconectado | 6,04:1 / 5,79:1 | 4,81:1 / 5,07:1 |
  | Pila verde, ámbar y roja | 7,11 / 7,16 / 5,39:1 | 5,08 / 4,87 / 5,36:1 |
  | Pila «sin respuesta» | 3,57:1 | 3,26:1 |
  | Contorno de la pila | 7,03:1 | 6,63:1 |
  | Texto sobre `surface.selected` | 13,47:1 | 14,46:1 |
  | Texto en campo (sobre `bg.app`) | 17,59:1 | 17,16:1 |
  | Borde de acción principal y de selección (`text.primary` sobre tarjeta) | 16,15:1 | 18,86:1 |

  Los bordes de los botones (1,60:1 en oscuro y 1,65:1 en claro) no portan identidad. El botón se identifica por su texto (≥ 4,5:1), así que se acepta. El botón deshabilitado, con texto a 3,72:1, está exento.

### Tipografía y espaciado

- **Familias y alternativas:** `Segoe UI Variable Text`, luego `Segoe UI` y luego la fuente del sistema. Es la tipografía de la plataforma, coherente con una app nativa.
- **Escala y pesos:**

  | Elemento | Tamaño y peso |
  | --- | --- |
  | Título | 26 px (22 px en estrecha), 600 |
  | Nombre | 17 px (16 px en estrecha), 600 |
  | Porcentaje | 22 px (20 px en estrecha), 600 |
  | «Con cable» | 16 px, 600 |
  | Cuerpo | 14 px, 400 |
  | Tipo | 13 px |
  | Etiqueta de sección | 12 px, 600, mayúsculas, espaciado 0,06 em |

- **Retícula, espaciado y anchuras:** contenido con ancho máximo de 1080 px, centrado y con márgenes de 32 px (16 px en estrecha). Rejilla `auto-fill` con columnas de 300 px como mínimo y separación de 16 px. Tarjeta con 20 px de relleno (16 px en estrecha), 16 px entre bloques y 280 px de alto mínimo en ancho intermedio o amplio.
- **Radio, bordes y profundidad:** tarjetas con radio 10, botones con 6 y aviso con 8. Bordes de 1 px y sin sombras.
- **Iconografía, imágenes o material visual:** solo los pictogramas propios descritos. Sin fotos ni logotipos de marcas.

### Patrones Apple adaptados

No aplican: no hay gestos, capas ni transiciones espaciales.

### Contrato de tokens semánticos

- **Convención técnica del proyecto:** aún no hay sistema de tokens en el código. La implementación los define como constantes con nombre, por tema, en un único módulo de la UI.
- **Tokens existentes reutilizados:** ninguno. La interfaz anterior solo usaba colores del sistema.
- **Roles nuevos aprobados y su propósito:**

| Token | Oscuro | Claro | Propósito |
| --- | --- | --- | --- |
| `bg.app` | `#0B0B0C` | `#F4F4F5` | Fondo de la ventana |
| `surface.card` | `#161618` | `#FFFFFF` | Tarjeta |
| `surface.card-disconnected` | `#111113` | `#FAFAFB` | Tarjeta de dispositivo desconectado |
| `border.card` | `#26262A` | `#E1E1E5` | Borde de tarjeta |
| `border.control` | `#3A3A3F` | `#C9C9CE` | Contorno de botón y de muestra de color |
| `border.notice` | `#2A2A2E` | `#DCDCE0` | Borde del aviso |
| `text.primary` | `#F2F2F3` | `#111113` | Texto principal e iconos |
| `text.secondary` | `#A1A1A6` | `#5C5C63` | Metadatos |
| `text.tertiary` | `#8E8E94` | `#6B6B72` | Etiquetas de sección y texto de desconectado |
| `text.notice` | `#C8C8CC` | `#3A3A40` | Texto del aviso |
| `text.disabled` | `#6E6E74` | `#8E8E94` | Botón deshabilitado |
| `focus.ring` | `#F2F2F3` | `#111113` | Contorno de foco de 2 px, separado 2 px |
| `battery.outline` | `#A1A1A6` | `#5C5C63` | Contorno y borne de la pila; icono de cable |
| `battery.empty` | `#2C2C30` | `#E1E1E5` | Franja apagada |
| `battery.high` | `#3FB950` | `#1A7F37` | 41–100 % |
| `battery.mid` | `#D29922` | `#9A6700` | 21–40 % |
| `battery.low` | `#F85149` | `#CF222E` | 0–20 % |
| `battery.stale` | `#6E6E74` | `#8E8E94` | Último valor conocido |
| `surface.selected` | `#26262A` | `#E1E1E5` | Valor rápido elegido; fila de botón en edición |
| `overlay.scrim` | `#000000` al 55 % | `#000000` al 55 % | Fondo tras un diálogo modal |

- **Valores locales excepcionales y motivo:** la muestra del color aplicado usa el color del dispositivo. Es un dato, no un token.
- **Fuera de la exploración visual:** `text.tertiary` y `surface.card-disconnected` en claro no se dibujaron, pero su contraste está medido (4,81:1 sobre fondo y 5,07:1 sobre tarjeta desconectada). La tarjeta desconectada se distingue del fondo por su borde y su texto, no por el relleno (1,05:1).

## Componentes mínimos y estados

- **Inventario necesario (ajustes del ratón):**
  - Botón «‹ Dispositivos».
  - Cabecera de dispositivo.
  - Sección.
  - Campo numérico con −/+.
  - Grupo de valores rápidos con estado presionado.
  - Tabla de botones con fila editable.
  - Selector nativo de acción.
  - Campo de captura de atajo.
  - Barra de pendientes.
  - Diálogo de confirmación modal.
  - Línea de estado por sección.
- **Inventario necesario (vista):**
  - Encabezado.
  - Aviso (información y error).
  - Tarjeta de dispositivo.
  - Pila de batería.
  - Icono por tipo y de cable.
  - Botón con contorno.
  - Panel vacío.
  - Panel de error.
  - Tarjeta de carga con forma final.
- **Descartados o no justificados:** chips, menú «…», selector de vista, barra lateral y notificaciones flotantes.

## Componentes y estados

| Elemento o flujo | Estado base | Carga/progreso | Vacío | Error y recuperación | Éxito | Deshabilitado | Foco, activo y sobreflujo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Vista | Encabezado y rejilla | Tarjetas con la forma final en bloques grises y el texto «Buscando dispositivos…» (`role=status`) | Panel con borde discontinuo: «No hay dispositivos compatibles conectados», explicación, «Actualizar» y «Ver dispositivos compatibles» | Panel `role=alert`: «Error: no se pudo acceder a los dispositivos», causa y «Reintentar» | Contador actualizado | — | Orden de tabulación: «Actualizar» y luego la tarjeta de cada dispositivo |
| Aviso de conflicto | Texto persistente con icono | — | Oculto si no hay conflicto | — | — | — | El texto largo pasa a varias líneas |
| Tarjeta | Ver orden fijo | — | — | «Sin respuesta» con el último valor y la pila `stale` | — | Desconectado: superficie y texto atenuados, «Ajustes» deshabilitado con su motivo en el nombre accesible | Nombre en 2 líneas con «…»; texto largo en varias líneas, sin cortar valores |
| Pila | Franjas y porcentaje | — | Sin dato: bloque omitido | Error de batería en texto | Carga completa: 5 franjas | — | — |
| Sección DPI | Valor actual, campo y valores rápidos | «Aplicando…», controles deshabilitados | — | Valor no válido (estado inválido, foco al campo) o sin respuesta («Reintentar») | Verificación y «aplicado y guardado» | Desconectado o en el límite (−/+) | El foco recorre −, campo, +, Aplicar y los valores rápidos |
| Sección Botones | Tabla con «Cambiar» | «Guardando en el ratón…», sección bloqueada | Ratón no compatible (sin controles) | Panel de error con «Reintentar» y «Restaurar perfil original…»; los cambios se conservan | «Botones guardados… comprobado» | Desconectado; Izquierdo «No se cambia» | Fila en edición resaltada; marca «Cambiado · antes: …» |
| Diálogo de confirmación | Lista de cambios y consecuencia | — | — | — | — | — | Foco inicial en «Cancelar»; Esc cancela; foco atrapado dentro mientras está abierto; al cerrar vuelve al disparador |
| Botón | Contorno y texto | — | — | — | — | `text.disabled`, sin puntero | Anillo `focus.ring` de 2 px separado 2 px; activo con fondo `surface` un tono más claro |

## Diseño adaptable y acceso equivalente

- **Compacto** (ventana de unos 420 px):
  - Una columna con márgenes de 16 px.
  - El icono va a la izquierda del nombre, y el tipo se integra en la línea de estado («Ratón · Conectado · …»).
  - «Ajustes» comparte fila con las capacidades.
  - Botones de 44 px de alto como objetivo táctil.
- **Intermedio** (de 720 a 1100 px): 2 o 3 columnas, según quepan las de 300 px mínimo.
- **Amplio:** contenido centrado con 1080 px como máximo; no se estira.
- **Teclado y foco:**
  - **Vista:** Tab recorre «Actualizar» y luego el «Ajustes» de cada tarjeta en orden de lectura. El foco siempre es visible. Esc cierra la ventana.
  - **Ajustes del ratón:** orden «‹ Dispositivos», −, campo DPI, +, Aplicar, valores rápidos, los «Cambiar» de cada botón, barra de pendientes, «Restaurar perfil original…» y «Restaurar originales». Enter en el campo aplica el DPI. Esc vuelve a la vista, salvo si hay un diálogo, que se cierra primero, o una fila en edición, que se cancela primero. Con cambios de botones pendientes, salir pide confirmar el descarte.
  - **Diálogos:** el foco queda atrapado dentro, empieza en «Cancelar» y al cerrar vuelve al botón que lo abrió.
  - **Captura de atajo:** Tab y Esc nunca se capturan.
- **Semántica y mensajes:**
  - Cada tarjeta es una región con el nombre del dispositivo.
  - La pila tiene nombre accesible «Nivel de batería: N de 5».
  - El estado se escribe siempre («Conectado», «Sin respuesta», «Desconectado», «Batería baja»).
  - Los errores llevan el prefijo «Error:».
- **No depender solo del color:** el nivel de batería se expresa con franjas, porcentaje y texto; la conexión, con texto; el foco, con anillo; el desconectado, con texto además del atenuado.
- **Movimiento:** ninguno. Si se añadiera, debe respetar la preferencia de Windows de reducir animaciones y no ser necesario para entender nada.
- **Tema:** la app sigue el tema de aplicaciones de Windows (claro u oscuro) y cambia en caliente. La barra de título acompaña al tema. Con alto contraste activo, deben prevalecer los colores del sistema (requisito para la implementación).

## Observaciones y aprobación

### Feedback incorporado

- «Negro y claro» significa dos temas que siguen a Windows.
- Icono de línea por tipo, sin fotos.
- Tarjetas en lugar de lista.
- Sin barra lateral.
- Batería como pila de colores con 5 franjas y porcentaje al lado.
- Icono propio para «con cable» en lugar de «—».
- El cable entra por el centro del enchufe.
- Icono de mando completo (cruceta y 4 botones).
- **Ajustes del ratón:**
  - Se abre en la misma ventana, con «‹ Dispositivos».
  - DPI con campo, −/+ y valores rápidos, aplicado al pulsar «Aplicar».
  - Tasa de sondeo en solo lectura, con explicación.
  - Mapeo de botones guardado **en la memoria del ratón** (opción B), con clics y navegación, multimedia, atajo de teclado y desactivar.

### Supuestos y pendientes

- **Viabilidad del mapeo de botones (bloqueante para implementarlo):** el diseño da por hecho que se puede leer, copiar, escribir, verificar y restaurar el perfil integrado del G305 (HID++ 0x8100). Nada de eso está comprobado, y en el plan v0.1 la escritura persistente estaba excluida por riesgo. El plan de implementación debe empezar por:
  - investigar el formato del perfil solo con lecturas;
  - guardar una copia verificada;
  - probar la restauración, con la aprobación expresa del usuario para cada escritura.

  Si resulta inviable o arriesgado, la sección «Botones» se rediseña o pasa a reasignación por software.
- Los valores rápidos 400, 800, 1600 y 3200 son una propuesta. Solo se muestran los que estén dentro del rango del ratón.

- El enlace «Ver dispositivos compatibles» apunta provisionalmente a la tabla del README, hasta que exista la URL pública del repositorio.
- La pantalla de ajustes de cada dispositivo está por diseñar.
- **Decisión técnica pendiente (planificación):** los controles clásicos actuales no admiten este diseño. Hay que elegir cómo dibujar la ventana a medida sin perder accesibilidad (nombres, foco y teclado), alto contraste ni bajo consumo. La elección afecta a rendimiento, accesibilidad y tamaño del binario, así que necesita su propio plan aprobado.

### Aprobación

- **Fecha:** 2026-10-06 (vista «Dispositivos») y 2026-10-06 (ajustes del ratón, «incorpóralo»).
- **Persona que aprueba:** el usuario del proyecto, en la sesión de diseño.
- **Alcance de la aprobación:** diseño. No incluye planificación ni implementación de los ajustes del ratón.
- **Observaciones:** el mapeo de botones queda condicionado a la verificación técnica descrita en «Supuestos y pendientes».

## Validación prevista para la implementación

- **Escenarios funcionales:**
  - Con G305, BlackWidow V3 TKL y DualSense reales: tarjetas, batería y estado correctos.
  - Desconexión y reconexión.
  - Dispositivo dormido.
  - Sin dispositivos.
  - Fallo de acceso HID simulado.
- **Responsive:** ventana a 420, 760 y 1280 px de ancho.
- **Teclado, foco y semántica:** recorrido completo con Tab y Esc. Árbol de UI Automation con nombre, rol y valor de cada tarjeta, pila y botón (herramienta UIA nativa ya usada en v0.1).
- **Contraste y estados:** tema oscuro, tema claro, alto contraste, escala al 200 % y nombres largos.
- **Movimiento reducido:** no aplica mientras no haya animaciones.
- **Capturas o revisión visual:** capturas de la app en ejecución en ambos temas, comparadas con este documento y con la exploración.
- **Ajustes del ratón:**
  - Aplicar, rechazar y restaurar el DPI con el G305 real.
  - Desconectar con la pantalla abierta.
  - Recorrido de teclado completo, incluidos diálogo y captura de atajo.
  - Árbol UIA con el estado presionado de los valores rápidos y el estado inválido del campo.
  - Para los botones: lectura, copia y restauración verificadas antes de cualquier escritura, y una escritura y restauración reales con el permiso expreso del usuario.
- **Recursos:** mantener los umbrales del plan v0.1 (working set ≤ 15 MB en reposo y CPU ≈ 0 %). Si se superan, hay que justificarlo.

## Evidencia de revisión en ejecución

| Fecha | Cambio o flujo | Entorno y ruta | Evidencia | Resultado | Desviación aprobada o seguimiento |
| --- | --- | --- | --- | --- | --- |
| 2026-10-06 | Vista «Dispositivos» completa (plan `Planes/2026-10-06-vista-dispositivos.md` v1) | Windows 11, 100 % de escala, binario release; G305, BlackWidow V3 TKL y DualSense reales | Capturas con PrintWindow: oscuro, claro (cambio en caliente con difusión de `WM_SETTINGCHANGE`), 1144, 760 y 420 px, foco con Tab | Conforme: tarjetas, pila de 5 franjas, cable, iconos, tokens, aviso, una a tres columnas y anillo de foco de 2 px | Ver «Desviaciones aprobadas» |
| 2026-10-06 | Accesibilidad y teclado | Árbol UIA nativo y `SendInput` | Panel con resumen completo; botones «Ajustes de X, próximamente»; Tab, Mayús+Tab, Enter, Espacio y Esc | Conforme con la desviación 1 | Narrador no probado por el usuario |
| 2026-10-06 | Desconexión real | El usuario desconecta un dispositivo con la vista abierta | Observación del usuario | Conforme: «Desconectado» y vuelve al reconectar | — |
| 2026-10-06 | Alto contraste y escala 150/200 % | — | — | Bloqueado/no verificado | Lo activará el usuario |

### Desviaciones aprobadas

1. **Rol de la tarjeta (aprobada por el usuario en el plan):** UI Automation la expone como «panel» cuyo nombre es el resumen completo, no como «región» con campos navegables por separado.
2. **Botón «Ajustes» como marcador (decisión del usuario, 2026-10-06):** se muestra, pero no hace nada hasta diseñar la pantalla de ajustes. Su nombre accesible añade «, próximamente». Las entradas por dispositivo del menú de la bandeja abren la vista.
3. **Barra de desplazamiento:** cuando el contenido no cabe, aparece la barra nativa, que es clara también en tema oscuro, porque oscurecerla requiere API no documentadas, excluidas por el plan. La ventana se abre con el ancho del diseño (3 columnas), así que con 1 a 3 dispositivos no se ve. Seguimiento: decidir si se dibuja una barra propia.

## Regresión visual automatizada

No aplica: el repositorio no usa Playwright, y la interfaz es nativa de Windows, no web.
