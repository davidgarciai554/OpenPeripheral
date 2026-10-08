# Diseño: OpenPeripheral — vista «Dispositivos» y ajustes del ratón

> Estado: aprobado (diseño). La vista «Dispositivos» y la pantalla «Ajustes del ratón» están implementadas, incluida la **revisión del 2026-10-07**: sección «Iluminación», DPI y luz guardados en la memoria del ratón y guardado común de la página (plan `Planes/2026-10-07-luz-y-guardado-raton-v2.md`, que modifica el v1).
>
> **Revisión del 2026-10-07 (tarde), aprobada:** perfiles del ratón guardados en el PC, que «Usar» escribe en el ratón; cambio desde la página y desde la bandeja; y perfiles por juego, que cambian solos al abrir y cerrar un juego.
> - **Perfiles y bandeja: implementados** (plan `Planes/2026-10-07-perfiles-raton.md`; ver «Evidencia» y las desviaciones 9 a 12).
> - **Perfiles por juego: implementados** (plan `Planes/2026-10-07-perfiles-por-juego.md`; desviaciones 13 a 16). Puertas de hardware HB1–HB4 pendientes.
>
> **Revisión del 2026-10-08, aprobada (maqueta) — «Ajustes del ratón» v2 y pantalla «Ajustes»:** asignación de botones sobre un dibujo del ratón con el panel «Asignaciones», perfiles en pastillas, DPI en panel lateral, pestañas Botones y DPI · Iluminación · Avanzado y una pantalla «Ajustes» general (plan `Planes/2026-10-08-raton-v2-asignaciones-y-ajustes.md`, v2). Ver la sección «Revisión 2026-10-08». **Donde contradiga a las secciones anteriores, prevalece esta revisión.** Implementación: hecha (incrementos 0 a 7), con las desviaciones 17 a 19; puertas de hardware HW1 y HW2 pendientes.
>
> Este documento es la fuente de verdad de diseño de la interfaz de OpenPeripheral. Debe leerse antes de modificarla. **Sustituye** la dirección anterior («controles clásicos de Windows, sin identidad propia»), aprobada en el plan v0.1, que no admitía tema oscuro.

## Propósito y alcance

- **Producto o área:** ventana principal de OpenPeripheral, una aplicación de escritorio para Windows 10/11.
- **Objetivo de la interfaz:** ver de un vistazo qué periféricos hay, si responden, su batería y lo que se puede ajustar en cada uno.
- **Persona, contexto y plataforma prioritaria:** quien usa periféricos de varias marcas en su PC con Windows y quiere evitar el software del fabricante. Suele tener de 1 a 6 dispositivos.
- **Trabajo principal:** comprobar la batería y el estado, y entrar a los ajustes de un dispositivo.
- **Incluido:**
  - Vista «Dispositivos» con encabezado, aviso de software en conflicto, rejilla de tarjetas, estados, tema oscuro y claro, y ventana estrecha.
  - Pantalla **«Ajustes del ratón»**: DPI, iluminación, botones, tasa de sondeo y perfil del ratón. DPI, luz y botones se guardan juntos en la memoria del ratón. Su estructura (cabecera común y secciones) será la base de teclados, cascos y mandos.
  - **Perfiles del ratón** (revisión del 2026-10-07, tarde): hasta 8 configuraciones con nombre (DPI, luz y botones), guardadas en el PC. Se cambia de perfil desde la página y desde el menú de la bandeja.
  - **Perfiles por juego:** al abrir un juego detectado se pregunta, con una notificación de Windows, si se quiere un perfil para él. Después, el perfil cambia solo al abrir y cerrar ese juego o programa.
- **Excluido (por ahora):**
  - Pantallas de ajustes de teclados, cascos y mandos. Su botón «Ajustes» sigue siendo un marcador sin acción (desviación 2).
  - Macros y el ciclo de DPI del botón DPI.
  - Atajos de teclado globales para cambiar de perfil, mostrar el perfil en la tarjeta de «Dispositivos», e importar o exportar perfiles.
  - Cambio automático de perfil con OpenPeripheral cerrado.
  - Navegación lateral.
  - Imágenes de producto (fotos o renders del fabricante). **Revisión 2026-10-08:** se admite un **dibujo esquemático propio** del ratón, vectorial y sin logotipos, para asignar botones.
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
| Mismo lienzo (v25): fila «Ajustes del ratón — sección Iluminación» (oscuro interactivo, claro, estados y 420 px) | Sección «Iluminación»; DPI y luz guardados en el ratón; «Probar» en lugar de «Aplicar»; barra de pendientes común con un diálogo y una escritura; sección «Perfil del ratón» | Revisión del 2026-10-07 descrita en «Pantalla Ajustes del ratón» | Aprobado el 2026-10-07 |
| Mismo lienzo (v30): fila «Ajustes del ratón — perfiles» (oscuro interactivo, claro con «Nuevo perfil», estados, 420 px y menú de la bandeja) | Perfiles guardados en el PC; «Usar» los escribe en el ratón sin diálogo; la página edita siempre el perfil activo | Sección «Perfiles» y «Perfiles en la bandeja» | Aprobado el 2026-10-07 |
| Mismo lienzo (v32): fila «Perfiles por juego» (página con un juego abierto; notificación, diálogo «Perfil para …», cambio automático, errores, «Añadir programa…» y límite) | Pregunta solo por juegos detectados, con una notificación; al cerrar el juego vuelve el perfil anterior | Sección «Juegos y programas» y «Flujo de perfiles por juego» | Aprobado el 2026-10-07 |
| https://claude.ai/artifact/Aa746hPCdQs417HoqWZHq1 (v12): «Ajustes del ratón · Botones y DPI» (interactivo), «Ventana estrecha · 420 px», «Pantalla Ajustes», «Dispositivos · acceso a Ajustes» y «Pestaña Avanzado» | Tres paneles (Asignaciones · ratón · DPI) como G HUB, perfiles en pastillas, pestañas de la página, pantalla de ajustes general | Toda la sección «Revisión 2026-10-08» | Aprobado el 2026-10-08 («apruebo esta maqueta») |

El enlace es solo una referencia. Este documento es la fuente de verdad.

## Dirección de diseño aprobada

- **Idea rectora:** instrumental y sobrio. Negro o claro, tipografía y espacio. El único color con significado es el **nivel de batería** y, como dato, el color de luz que el usuario elige o ha aplicado.
- **Personalidad y tono:** una herramienta discreta y precisa. Textos cortos en español, descriptivos, sin jerga.
- **Densidad de información:** media. Una tarjeta por dispositivo con 5 bloques: tipo, identidad, energía, capacidades y acción.
- **Principio de composición:** rejilla de tarjetas centrada con ancho máximo. El orden de lectura es de arriba abajo dentro de cada tarjeta.
- **Dirección que se evita:** acentos de marca, gradientes, sombras, cristal o desenfoque, fotos de producto, avisos flotantes que desaparecen y métricas decorativas.
- **Riesgo visual deliberado y motivo:** colores semánticos en la pila (verde, ámbar, rojo) dentro de una interfaz monocroma. Lo pidió el usuario y hace que la batería baja salte a la vista. El color nunca va solo: lo acompañan el número de franjas, el porcentaje y el texto «Batería baja».

## Arquitectura e interacción

- **Navegación y orientación:** una sola ventana. «Ajustes» en la tarjeta de un ratón sustituye la vista «Dispositivos» por «Ajustes del ratón», **en la misma ventana**. El botón «‹ Dispositivos» (y Esc) vuelve, con el foco devuelto al «Ajustes» de esa tarjeta. La bandeja del sistema sigue siendo el acceso principal, y desde ella también se cambia de perfil (ver «Perfiles en la bandeja»). «Configurar…», en la notificación de un juego nuevo, abre «Ajustes del ratón» con el diálogo «Perfil para <juego>».
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
4. Capacidades en texto: «1000 DPI · 1000 Hz», o «Color aplicado» con una muestra de 14 px y el valor hex. En teclados y mandos es siempre el **último color aplicado por la app**, porque el dispositivo no permite leer el suyo. En un ratón con luz se añade la luz guardada en su perfil: «1600 DPI · 1000 Hz · Luz fija ■ #00C8FF» («Luz apagada» o «Luz en ciclo de colores», sin muestra).
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
  3. Aviso de software en conflicto (solo si lo hay; p. ej. «Puede sobrescribir el DPI y la luz»).
  4. Texto en `text.secondary`: «Los ajustes se guardan en la memoria del ratón: funcionan aunque cierres OpenPeripheral y en otros PC. “Probar” lo envía al ratón sin guardarlo».
  5. **Perfiles** (revisión del 2026-10-07, tarde).
  6. **Sensibilidad (DPI).**
  7. **Iluminación** (solo si el ratón declara luz).
  8. **Botones.**
  9. **Tasa de sondeo.**
  10. **Juegos y programas** (revisión del 2026-10-07, tarde).
  11. **Restaurar** (antes «Perfil del ratón»; se renombra para no confundirla con «Perfiles»).
  12. **Barra de cambios sin guardar**, fija al pie de la ventana (solo si hay cambios).
- Con perfiles, el texto del punto 4 pasa a ser: «Cada perfil guarda el DPI, la luz y los botones. «Usar» escribe el perfil en la memoria del ratón, así que funciona aunque cierres OpenPeripheral y en otros PC. «Probar» lo envía al ratón sin guardarlo».
- **Énfasis de la acción principal** («Guardar en el ratón…», «Guardar en el ratón» en el diálogo, «Reintentar»): borde `text.primary` y texto seminegrita, sin color de acento. «Probar» y las demás acciones usan el botón con contorno estándar.
- Antes de la revisión del 2026-10-07 el orden era: DPI, Botones, Tasa de sondeo y Restaurar, con la nota final «Los cambios no se guardan en la memoria del ratón: OpenPeripheral los vuelve a aplicar al reconectarlo». Esa nota desaparece.

#### Modelo de guardado (revisión 2026-10-07)

- DPI, luz y botones viven en el **mismo perfil integrado** del ratón. Todos los cambios de la página se acumulan como pendientes y se escriben **juntos, con una sola confirmación y una sola escritura**, verificada releyendo el perfil.
- **«Probar»** (DPI e iluminación) envía el valor al ratón sin guardarlo, para notarlo o verlo. Lo probado no cuenta como guardado. Al descartar, salir sin guardar o reconectar el ratón, vuelve lo guardado.
- Cada sección con cambios muestra en su título la etiqueta con contorno «Cambiado · antes: <valor guardado>». Junto a «Probar» aparece «Guardado: <valor>».
- **Barra de cambios sin guardar:** fija al pie de la ventana, sobre `surface.card` con borde superior `border.control`. Muestra «**N cambios sin guardar.** Todavía no se han escrito en el ratón», con «Descartar» y «Guardar en el ratón…». Si se sale con cambios pendientes, se pregunta si se descartan.
- **Confirmación:** el diálogo de «Botones» pasa a ser el de toda la página: «¿Guardar N cambios en la memoria del ratón?», con la lista de cambios («DPI al arrancar: 1000 → 1600», «Luz: fija, Cian #00C8FF → respiración, Naranja #FF8000, lenta», «Lateral trasero: Atrás → Ctrl + C»). Mantiene el texto, el foco y los botones.
- **Guardando, guardado y error:** los estados de «Botones» pasan a la página. La barra muestra «Guardando en el ratón…», luego la marca de verificación con «Guardado en el ratón. Se comprobó la escritura leyendo de nuevo el perfil», o el panel de error, que conserva los cambios.
- **Bloqueo por G HUB:** con G HUB en ejecución se deshabilitan «Guardar en el ratón…», «Restaurar perfil original…» y «Valores de fábrica…». «Probar» sigue disponible.
- **Aplicar al conectar (plan v2, decisión del usuario):** la app recuerda en su configuración el DPI y la luz guardados y los envía en vivo, sin escribir en la memoria, al iniciarse o al conectar el ratón. Así se impone aunque otro programa los haya cambiado. «Probar» no se recuerda.
- **DPI guardado (plan v2, decisión del usuario):** la posición del ciclo con la que arranca el ratón no está en el perfil, así que el DPI guardado se escribe en los **4 niveles** del botón DPI; la confirmación lo dice. «Valores de fábrica…» devuelve el ciclo de fábrica.
- **«Restaurar originales»** (en «Perfil del ratón») deja de aplicar el DPI y la luz de la app al conectar, vuelve al DPI que tenía el ratón al detectarlo y a la luz de su perfil. No escribe en el ratón.
- Texto común de la página: «OpenPeripheral aplica tu DPI y tu luz al iniciarse o al conectar el ratón. “Guardar en el ratón…” los conserva también sin la app y en otros PC. “Probar” lo envía al ratón sin guardarlo».

#### Perfiles (revisión 2026-10-07, tarde)

- **Por qué en el PC:** el G305 solo tiene **1 perfil integrado** (`getInfo` de `0x8100`; ver `docs/protocolos/logitech-hidpp.md`). Los perfiles viven en la configuración de la app. Son de cada ratón concreto y no se comparten entre modelos.
- **Modelo (decisión del usuario):**
  - Un perfil guarda el DPI al arrancar, la luz y los botones.
  - **«Usar» escribe el perfil en la memoria del ratón** al momento, **sin diálogo de confirmación**, con la misma escritura verificada por relectura que «Guardar en el ratón».
  - **La página edita siempre el perfil activo**: lo que se ve es lo que hay en el ratón. Para editar otro perfil, primero se usa. «Guardar en el ratón…» actualiza a la vez el ratón y el perfil activo.
  - «Restaurar perfil original…» y «Valores de fábrica…» también actualizan el perfil activo.
  - Se descartó editar un perfil inactivo sin activarlo, porque mezcla dos fuentes de verdad en la misma página.
- **Título:** «Perfiles», con el contador «N de 8» en `text.tertiary`.
- **Lista** (`ul` con nombre accesible «Perfiles de este ratón»):
  - Cada fila lleva radio 8 y relleno de 12×14 px: el nombre (15 px, seminegrita) y el resumen en `text.secondary`, por ejemplo «1600 DPI · Luz respiración rojo #FF0000, lenta · Botones: Lateral trasero → Ctrl + C». Si el perfil tiene programas asociados, añade «Se activa con: Counter-Strike 2, Elden Ring».
  - **Fila activa:** fondo `surface.selected`, borde `text.primary`, la marca de verificación y «**Activo · en el ratón**» en seminegrita, y el botón «Renombrar…». Nunca se distingue solo por el color.
  - **Filas inactivas:** «Usar» (énfasis de acción principal) y «Eliminar…». Nombres accesibles: «Usar el perfil Juego» y «Eliminar el perfil Juego».
  - Se descartó un grupo de opción única, porque las flechas escribirían en el ratón en cada pulsación.
- **Pie:** «Nuevo perfil…» y el texto «Copia el perfil activo. Hasta 8 perfiles».
- **Primera vez:** lo que hay en el ratón se convierte en «Perfil 1», activo, con el texto «Crea otro perfil para cambiar rápido entre configuraciones, por ejemplo una para trabajar y otra para jugar».
- **Diálogos** (mismo patrón modal, con el foco inicial en el primer campo o en «Cancelar»):
  - **Nuevo perfil:** «Nombre» (prellenado, por ejemplo «Juego 2»; hasta 32 caracteres, único) con la ayuda «Hasta 32 caracteres. No puede repetir el nombre de otro perfil», y el texto «Empieza como una copia de «Juego»: … No se escribe en el ratón hasta que pulses “Usar”». Botones: Cancelar / Crear perfil.
  - **Renombrar «Trabajo»:** si el nombre está repetido, el campo lleva borde de 2 px y estado inválido, con «**Error:** ya hay un perfil llamado «Juego». Elige otro nombre», y «Renombrar» queda deshabilitado.
  - **¿Eliminar el perfil «Diseño»?:** «Se borra de este PC. El ratón no cambia, porque «Diseño» no es el perfil activo». Si tiene programas asociados, se añade que pasarán a «No cambiar de perfil». Botones: Cancelar (foco inicial) / Eliminar perfil. El perfil activo no tiene «Eliminar…», y siempre queda al menos uno.
  - **Cambios sin guardar al pulsar «Usar»:** «“Trabajo” tiene 1 cambio sin guardar», con la lista de cambios y «Antes de cambiar a “Juego”, elige qué hacer con él. Si lo descartas, se pierde». Botones: Cancelar (foco inicial) · Descartar y cambiar · Guardar y cambiar.
- **Estados:**
  - **Cambiando:** «Cambiando a «Juego»… No apagues ni desconectes el ratón hasta que termine» (`role=status`). La página entera se bloquea, también «‹ Dispositivos». Tarda uno o dos segundos.
  - **Éxito:** la marca de verificación y «Perfil «Juego» activo en el ratón. Se comprobó la escritura leyendo de nuevo el perfil». DPI, luz y botones pasan a los del perfil nuevo.
  - **Error** (panel `role=alert`): «**Error:** no se pudo cambiar a «Juego». El ratón no respondió», y «El ratón sigue con «Trabajo»: la comprobación de la escritura falló y OpenPeripheral volvió a escribir lo que había. Muévelo para despertarlo y vuelve a intentarlo», con «Reintentar».
  - **G HUB en ejecución:** «Usar» se deshabilita (nombre accesible «…, cierra G HUB para cambiar de perfil»), con el aviso «Cierra G HUB para cambiar de perfil. Puedes crear, renombrar y eliminar perfiles igualmente».
  - **Desconectado:** «Usar» se deshabilita, con «Conecta o enciende el ratón para cambiar de perfil. Los cambios sin guardar se conservan».
  - **El ratón no coincide con el perfil activo:** al abrir la página se compara el perfil del ratón con el activo. Si otro programa lo ha cambiado, aparece el aviso «El ratón tiene ajustes distintos de «Trabajo»: DPI al arrancar 3200 (en el perfil: 1000). Puede que otro programa lo haya cambiado», con «Volver a aplicar «Trabajo»» y «Guardar como perfil nuevo…». La fila activa muestra «Activo · el ratón no coincide»: la marca «en el ratón» solo aparece si coinciden.
  - **Límite:** con 8 perfiles, «Nuevo perfil…» se deshabilita, con «Has llegado a 8 perfiles. Elimina uno para crear otro».
- **Perfil activado por un juego:** aviso con el pictograma de mando: «**VALORANT está abierto:** se usa el perfil «VALORANT». Al cerrarlo, el ratón vuelve a «Trabajo»». Si se usa otro perfil con el juego abierto, se mantiene hasta que se cierre el juego («Si usas otro perfil con el juego abierto, se mantiene hasta que lo cierres»).

#### Juegos y programas (revisión 2026-10-07, tarde)

- **Descripción:** «Al abrir uno de estos programas, OpenPeripheral escribe su perfil en el ratón. Al cerrarlo, vuelve el perfil que había. Solo funciona con OpenPeripheral abierto».
- **Casillas nativas:**
  - «Cambiar de perfil al abrir estos programas».
  - «Preguntar al detectar un juego nuevo», con la ayuda «Juegos de Steam, Xbox, Epic o GOG. Se pregunta con una notificación de Windows» (sin pantalla completa: desviación 13).
- **Tabla** (en un contenedor con desplazamiento horizontal si no cabe):
  - Columna «Programa»: el nombre en seminegrita, «· abierto» si lo está, y el .exe en `text.secondary`.
  - Columna «Perfil»: un selector nativo con los perfiles y «No cambiar de perfil».
  - «Quitar», con nombre accesible «Quitar VALORANT».
  - Nota: «“No cambiar de perfil” es lo que se guarda al responder “No preguntar más”. “Quitar” hace que se vuelva a preguntar la próxima vez».
- **«Añadir programa…»:** para programas que no se detectan solos («como un editor o el navegador»). Abre un diálogo con:
  - la lista de programas abiertos (`listbox`, con nombre y .exe);
  - el selector «Perfil»;
  - «Buscar archivo…» y Cancelar / Añadir.

#### Flujo de perfiles por juego (revisión 2026-10-07, tarde)

Decisiones del usuario: se pregunta **solo por juegos detectados**, la pregunta se hace con una **notificación de Windows** y al cerrar el juego **vuelve el perfil anterior**.

1. **Juego nuevo:** un programa de las bibliotecas de Steam, Epic, GOG o Xbox sin asociación previa (la pantalla completa se descartó: desviación 13). Aparece la notificación «Juego nuevo: VALORANT. ¿Quieres un perfil del G305 para cuando lo abras?», con tres botones:
   - **Configurar…**
   - **Ahora no:** vuelve a preguntar la próxima vez que se abra.
   - **No preguntar más:** lo guarda como «No cambiar de perfil».

   La notificación no le quita el foco al juego. En modo juego o con «No molestar», espera en el centro de notificaciones. Se pregunta una sola vez por cada apertura del juego.
2. **«Configurar…»** abre «Ajustes del ratón» con el diálogo «**Perfil para VALORANT**» y el .exe debajo del título. Ofrece un grupo de opción única:
   - «Crear un perfil nuevo» (elegido por defecto), con el campo «Nombre» prellenado con el nombre del juego y «Empieza como copia de «Trabajo». Después ajusta su DPI, su luz y sus botones en esta página».
   - «Usar un perfil que ya tengo», con un selector nativo.

   El texto del diálogo dice: «Como VALORANT está abierto, el perfil se escribe en el ratón ahora. Al cerrarlo volverá «Trabajo»». Botones: Cancelar / Crear y usar. Con 8 perfiles, «Crear un perfil nuevo» se deshabilita, con «Has llegado a 8 perfiles. Usa uno que ya tengas o elimina alguno», y queda elegida la otra opción.
3. **Las siguientes veces:** al abrir el juego se escribe su perfil y al cerrarlo se vuelve a escribir el que había (2 escrituras por partida). Si funciona, no hay aviso: lo indican la página y la información del icono de la bandeja («G305: 90 % · perfil VALORANT (por juego)»). Si hay dos juegos con perfil abiertos, manda el último que se abrió.
4. **Fallos**, siempre con una notificación de Windows que dice con qué perfil sigue el ratón:
   - «No se cambió al perfil «VALORANT». G HUB está abierto y podría sobrescribirlo. El ratón sigue con «Trabajo»».
   - «No se pudo volver a «Trabajo». El ratón no respondió y sigue con «VALORANT». Muévelo: OpenPeripheral lo intentará de nuevo al detectarlo».
   - Con el ratón desconectado no se avisa: el perfil se escribe en cuanto vuelva, si el juego sigue abierto.

#### Perfiles en la bandeja (revisión 2026-10-07, tarde)

- **Submenú nativo** debajo de la línea de cada ratón: «Perfil de G305: **Trabajo** ›». Solo aparece si el ratón tiene al menos 2 perfiles.
  - Dentro, los perfiles como elementos de opción única, con la marca en el activo, un separador y «Editar perfiles…», que abre «Ajustes del ratón».
  - Elegir un perfil lo escribe en el ratón al momento.
- **Bloqueado** (con G HUB abierto o el ratón desconectado): la primera línea, en gris, da el motivo («Cierra G HUB para cambiar de perfil» o «Conecta o enciende el ratón para cambiar de perfil»), y los perfiles aparecen en gris. «Editar perfiles…» sigue activo.
- **Resultado:** si el cambio falla, una notificación de Windows («No se pudo cambiar a «Juego». El ratón no respondió y sigue con «Trabajo». Muévelo y vuelve a intentarlo»), que se queda en el centro de notificaciones. Si funciona, no hay aviso: la información del icono añade «· perfil Trabajo».
- El menú es el nativo de Windows. Las mesas solo lo dibujan para mostrar su contenido.

#### Sensibilidad (DPI)

- Título, y una descripción con el rango leído del ratón: «DPI con el que arranca el ratón. Admite de 200 a 12000, en pasos de 50. El botón DPI sigue recorriendo el resto del ciclo del perfil».
- **Revisión 2026-10-07:**
  - El valor se guarda en el perfil como **DPI al arrancar**; el resto del ciclo del botón DPI no cambia.
  - Donde esta sección dice «Aplicar», ahora es **«Probar»** (botón estándar), y «Actual: N DPI» pasa a «Guardado: N DPI».
  - Los estados de aplicar se leen como estados de probar: «Probando 1600 DPI en el ratón. Guárdalo para que se mantenga al apagarlo».
  - El cambio se guarda desde la barra común.
- **Nuevo valor:** etiqueta, botón «−», campo numérico (96 px; ocupa el ancho en estrecha) y botón «+». Los botones suman o restan un paso (50) y se deshabilitan en el mínimo o el máximo, con nombre accesible «Subir 50 DPI (máximo alcanzado)». El campo tiene fondo `bg.app` y borde `border.control`.
- **Aplicar** al lado del campo y **«Actual: N DPI»** en texto. El DPI **solo se envía al pulsar «Aplicar»** (o Enter en el campo).
- **Valores rápidos:** 400 · 800 · 1600 · 3200 (solo los que estén dentro del rango del ratón). Rellenan el campo, no aplican. El elegido se marca con fondo `surface.selected`, borde `text.primary`, seminegrita y estado presionado, nunca solo con color. En estrecha se muestran en una rejilla de 4.
- **Línea de estado** de la sección:
  - Sin aplicar: «Pulsa “Aplicar” para enviar 1600 DPI al ratón».
  - Aplicando: «Aplicando 1600 DPI…», con los controles deshabilitados.
  - Éxito: marca de verificación y «DPI 1600 aplicado y guardado. Se volverá a aplicar al reconectar el ratón».
  - Valor no válido: icono de error y «**Error:** 1625 no es válido. Usa un valor de 200 a 12000 en pasos de 50 (por ejemplo, 1600 o 1650)». El campo lleva borde de 2 px y estado inválido, y el foco vuelve a él.
  - Sin respuesta: «**Error:** el ratón no respondió; sigue en 1000 DPI. Muévelo para despertarlo y vuelve a intentarlo», con «Reintentar».

#### Iluminación (revisión 2026-10-07)

- Solo aparece si el ratón declara luz (HID++ `0x8070`), y solo ofrece los efectos que el ratón declara. Descripción: «Una zona de luz. Este ratón admite: apagada, fija, ciclo de colores y respiración».
- **Efecto:** grupo de opción única (`radiogroup`) con botones de 36 px: Apagada · Fija · Ciclo de colores · Respiración. El elegido usa el estilo del valor rápido elegido: fondo `surface.selected`, borde `text.primary`, seminegrita y estado marcado.
- **Color** (solo con Fija y Respiración):
  - Muestras de 32 px (44 px en estrecha), con radio 6 y borde `border.control`. La elegida lleva el anillo de 2 px `text.primary` separado 2 px y el estado presionado. Nombre accesible: «Cian, #00C8FF».
  - Muestras iniciales: Blanco #FFFFFF, Rojo #FF0000, Naranja #FF8000, Amarillo #FFD000, Verde #00FF40, Cian #00C8FF, Azul #0040FF y Morado #A000FF. **Son una lista de datos ampliable:** la fila se ajusta al ancho, así que añadir muestras no cambia la composición.
  - Campo «Hex» (112 px; ocupa el ancho en estrecha) con fondo `bg.app`. Admite «#RRGGBB» o «RRGGBB». A su lado, una vista previa de 36 px y el texto «Cian · #00C8FF» o «Personalizado · #12AB34». El color nunca se identifica solo por la muestra.
- **Velocidad** (solo con Ciclo y Respiración): grupo de opción única Lenta · Media · Rápida. También es una lista ampliable; los periodos concretos se fijan en el plan, dentro de lo que admita el ratón.
- **Textos fijos:**
  - Apagada: «Con la luz apagada no hay color ni velocidad que elegir».
  - Ciclo: «El ciclo recorre todos los colores; solo se elige la velocidad».
- Pie de la sección: «Probar» y «Guardado: Fija, Cian #00C8FF».
- **Estados:**
  - Sin cambios: sin línea de estado.
  - Cambiado: «Pulsa “Probar” para verlo en el ratón, o guárdalo directamente».
  - Probando: «Enviando la luz … al ratón para probarla…», con los controles deshabilitados; después, «Probando en el ratón: …. Guárdalo para que se mantenga al apagarlo».
  - Color no válido: el campo lleva borde de 2 px y estado inválido, con «**Error:** «#00C8F» no es un color válido. Escribe 6 cifras hexadecimales, por ejemplo #00C8FF». La vista previa queda vacía, con borde discontinuo y «Sin vista previa». No se puede probar ni guardar.
  - Sin respuesta: «**Error:** el ratón no respondió; la luz del ratón no ha cambiado. Muévelo para despertarlo y vuelve a intentarlo», con «Reintentar».
  - Ratón con luz no verificada: borde discontinuo y «Este ratón todavía no permite cambiar su luz desde OpenPeripheral». Si el ratón no declara luz, la sección no se muestra.
  - Desconectado: controles deshabilitados y «Conecta o enciende el ratón para cambiar sus ajustes. Los cambios sin guardar se conservan».

#### Botones

- Descripción (antes de la revisión): «Se guardan en la memoria del ratón: funcionan aunque cierres OpenPeripheral y en otros PC». Con la revisión, esta frase pasa al texto común de la página.
- **Tabla** de botones: nombre (Izquierdo, Derecho, Central (rueda), Lateral trasero, Lateral delantero, Botón DPI), acción actual y «Cambiar». La lista sale del perfil del ratón; el G305 declara 6 botones.
  - **Izquierdo:** «No se cambia», sin botón. Nunca se puede reasignar ni desactivar.
  - **Botón DPI:** muestra «Ciclo de DPI del perfil» mientras no se conozca su ciclo real.
- **Edición** («Cambiar»): la fila se expande con fondo `surface.selected`, contorno `border.control` y radio 8.
  - Selector nativo **«Acción»** con tres grupos:
    - Clics y navegación: Clic izquierdo, Clic derecho, Clic central, Atrás, Adelante, Doble clic.
    - Multimedia: Reproducir / pausa, Siguiente, Anterior, Subir volumen, Bajar volumen, Silenciar.
    - Otras: Atajo de teclado…, Desactivar.
    - **Implementado (desviación 4):** solo Clic izquierdo, Clic derecho, Clic central, Atrás, Adelante y Atajo de teclado…, cuyo formato se ha observado en el ratón. Doble clic, Multimedia y Desactivar se añadirán cuando haya evidencia de su codificación. Si la acción actual es una función del perfil que no se puede escribir, aparece «Sin cambios: …».
  - Con **«Atajo de teclado…»** aparece un campo de captura (de solo lectura, que muestra «Ctrl + C») con «Borrar». Ayuda: «Con el campo seleccionado, pulsa la combinación. Tab y Esc no se capturan: sirven para salir del campo».
  - «Aceptar» y «Cancelar» cierran la edición de la fila.
- **Cambio pendiente:** la fila muestra la acción nueva en seminegrita y la etiqueta con contorno «Cambiado · antes: Adelante».
- **Barra de pendientes.** Con la revisión del 2026-10-07 pasa a ser la barra común de la página, fija al pie de la ventana (ver «Modelo de guardado»). Antes estaba al pie de la sección y aparecía solo si había cambios: «**N cambios sin guardar.** Todavía no se han escrito en el ratón», con «Descartar» y «Guardar en el ratón…». Al salir con cambios pendientes se pregunta si se descartan.
- **Confirmación** (diálogo modal sobre `overlay.scrim`):
  - Título «¿Guardar N cambios en la memoria del ratón?», con la lista de cambios.
  - Texto: «El ratón los conservará aunque cierres OpenPeripheral o lo uses en otro PC. Si la comprobación de la escritura falla, OpenPeripheral deja el perfil como estaba» (desviación 5).
  - Botones «Cancelar» (con el foco inicial) y «Guardar en el ratón». Esc cancela; al cerrar, el foco vuelve a «Guardar en el ratón…».
- **Guardando:** «Guardando en el ratón… No apagues ni desconectes el ratón hasta que termine». La sección se bloquea.
- **Guardado:** marca de verificación y «Botones guardados en el ratón. Se comprobó la escritura leyendo de nuevo el perfil».
- **Error:** panel de alerta con «**Error:** no se pudieron guardar los botones», la causa, «Tus cambios siguen pendientes en esta pantalla», y los botones «Reintentar» y «Restaurar perfil original…».
- **Copia y restauración:** «Copia del perfil original guardada en este PC el <fecha>» y «Restaurar perfil original…», con su propia confirmación («Se escribirá en el ratón la copia guardada el <fecha>. Se perderán los botones que hayas cambiado»; Cancelar / Restaurar).
- **Valores de fábrica…** (añadido por decisión del usuario en el plan): copia los botones del perfil de fábrica del ratón al perfil de usuario y deja el resto igual. Tiene su propia confirmación («¿Volver a los botones de fábrica?»; Cancelar / Restaurar botones de fábrica).
- **Revisión 2026-10-07 (decisión del usuario):** «Restaurar perfil original…» y «Valores de fábrica…» pasan a la sección «Perfil del ratón» y restauran a la vez el **DPI al arrancar, la luz y los botones**. Textos de la confirmación de fábrica: «¿Volver a los valores de fábrica?» y «Se escribirán en el ratón el DPI, la luz y los botones de fábrica»; Cancelar / Restaurar valores de fábrica.
- **Bloqueo por G HUB** (decisión del usuario): con G HUB en ejecución, «Guardar en el ratón…», «Restaurar perfil original…» y «Valores de fábrica…» se deshabilitan y aparece «Cierra G HUB para guardar los botones». El servicio `lghub_updater` no cuenta.
- **Ratón no compatible**, o protocolo no verificado: borde discontinuo y «Este ratón todavía no permite cambiar sus botones desde OpenPeripheral». No se muestra ningún control que no funcione.

#### Tasa de sondeo y Restaurar

- **Tasa de sondeo:** «1000 Hz · solo lectura» y «No se puede cambiar mientras el ratón usa sus perfiles integrados».
- **Restaurar** (sustituida en la revisión del 2026-10-07): «Vuelve al DPI que tenía el ratón al detectarlo (N) y olvida los ajustes guardados» y «Restaurar originales». Con el DPI guardado en el ratón deja de tener sentido y la sustituye:
- **Perfil del ratón:** «Copia del perfil original guardada en este PC el <fecha>. Incluye el DPI, la luz y los botones», con «Restaurar perfil original…» y «Valores de fábrica…», cada uno con su confirmación.
- **Revisión del 2026-10-07 (tarde):** la sección pasa a llamarse **«Restaurar»**, con el texto «Copia del perfil original del ratón guardada en este PC el <fecha>. Lo que restaures se guarda en el perfil activo, «<perfil>»».

#### Ratón desconectado con la pantalla abierta

La cabecera pasa a «Desconectado» en `text.tertiary` y todos los controles se deshabilitan. Aparece el texto «Conecta o enciende el ratón para cambiar sus ajustes. Los controles se activarán solos cuando vuelva». No se cierra la pantalla, y los cambios sin guardar (DPI, luz y botones) se conservan.

### Revisión 2026-10-08: «Ajustes del ratón» v2 y pantalla «Ajustes»

Decisiones del usuario y de la mesa redonda del 2026-10-08; la maqueta aprobada está en «Exploración visual». Esta revisión prevalece sobre las secciones anteriores de «Pantalla «Ajustes del ratón»» donde las contradiga. **El modelo de guardado no cambia:** todo lo que se cambia queda pendiente y se guarda con la barra común, una confirmación y una escritura verificada.

#### Ventana y composición

- La ventana se abre a **1440×900** (unidades de diseño a 96 ppp), limitada al área de trabajo. El mínimo sigue en 360×400.
- **Orden de «Ajustes del ratón»**, de arriba abajo:
  1. «‹ Dispositivos».
  2. Cabecera: nombre (26 px, 600), «Ratón · Conectado · Receptor LIGHTSPEED · 1000 Hz» y la pila a la derecha. La tasa de sondeo pasa aquí, como dato de solo lectura.
  3. Aviso de software en conflicto (si lo hay y está activado en «Ajustes»).
  4. **Perfiles**, en pastillas.
  5. **Pestañas** de la página: «Botones y DPI» · «Iluminación» · «Avanzado».
  6. Contenido de la pestaña.
  7. Barra de cambios sin guardar, fija al pie (la misma que antes).
- **Anchuras de «Botones y DPI»:**
  - **≥ 1290 px de cliente:** tres paneles en fila: **Asignaciones (320)** · **ratón (≥ 560, flexible)** · **DPI (300)**, con 20 px entre ellos y márgenes de 32 px.
  - **De 760 a 1289:** Asignaciones y ratón en fila, y el DPI debajo del ratón.
  - **< 760 (estrecha):** todo apilado: el dibujo escalado **sin líneas guía**, la **lista de botones**, Asignaciones, DPI y la barra. Las pestañas de la página van en una rejilla de 3.
- «Iluminación» y «Avanzado» usan la columna centrada de 760 px de antes.

#### Perfiles en pastillas

- Etiqueta «PERFILES» (estilo de etiqueta de sección) y una pastilla por perfil: radio 18, relleno 7×14, borde `border.control`.
- **Pastilla activa:** fondo `surface.selected`, borde `text.primary`, seminegrita y «✓» delante del nombre.
  - Nombre accesible: «Perfil Trabajo, activo en el ratón».
  - Cambia a «…, el ratón no coincide» o «…, por juego» según los estados de «Perfiles».
- **Pastilla inactiva:** pulsarla equivale a «Usar». Escribe el perfil en el ratón con el mismo flujo, estados y errores, incluido el diálogo de cambios sin guardar.
  - Nombre accesible: «Usar el perfil Juego».
  - Son botones con Tab, **no** un grupo de opción única: las flechas escribirían en el ratón.
- **«+ Nuevo»** (borde discontinuo) abre el diálogo «Nuevo perfil». Con 8 perfiles se deshabilita, con el motivo en su nombre accesible.
- **«Perfil …»** abre un menú nativo de Windows con:
  - «Renombrar «X»…», «Guardar como perfil nuevo…» y «Volver a aplicar «X»».
  - Un separador, «Restaurar perfil original del ratón…» y «Valores de fábrica…». Mantienen sus confirmaciones y se deshabilitan con G HUB abierto.
  - Un separador, el texto en gris «Eliminar un perfil (el activo no se puede)» y «Eliminar «Y»…» por cada perfil inactivo.
- Las 8 pastillas saltan de línea y desaparece el paginador: esto sustituye a la desviación 16.
- El texto «Pulsar un perfil lo escribe en el ratón» va a la derecha de las pastillas (debajo, en estrecha).
- Los avisos de «Perfiles» van bajo las pastillas, con los mismos textos: por juego, el ratón no coincide, G HUB, desconectado, cambiando y error.

#### Pestañas de la página

- **Aspecto:** botones con texto de 14 px.
  - La elegida: `text.primary`, seminegrita y subrayado de 2 px `text.primary`.
  - Las demás: `text.secondary`.
- **Teclado:** un solo Tab para todo el grupo y flechas dentro de él, porque cambiar de pestaña no escribe nada. Nombre accesible: «Botones y DPI, elegida».
- Los pendientes de todas las pestañas siguen en la barra común: cambiar de pestaña no los descarta.

#### Botones y DPI: panel «Asignaciones»

- **Panel:** superficie `surface.card`, radio 10 y relleno 16.
- **Cabecera:** título «Asignaciones» (17 px, 600) y la línea «Para: **Lateral trasero** · ahora Ctrl + C · Copiar».
- **Pestañas del panel:** COMANDOS · TECLAS · RATÓN, a 13 px, 600, en mayúsculas y con subrayado de 2 px en la elegida. Solo aparecen las que el ratón admite: Macros y Sistema no se muestran.
- **Comandos:**
  - Campo «Buscar un comando», con fondo `bg.app`.
  - Recuento en vivo (`role=status`): «N comandos» o «Sin resultados para «…»».
  - Lista con desplazamiento propio, agrupada en Edición · Navegador y pestañas · Zoom · Ventanas.
  - Cada fila lleva el atajo (seminegrita, 120 px), el nombre en `text.secondary` y, a la derecha, el botón que la tiene asignada («Lateral trasero»), como «G4» en G HUB.
  - La fila asignada al botón elegido lleva fondo `surface.selected` y borde `text.primary`.
  - Pulsar una fila (o Intro) la asigna.
  - Grupos: Edición · Navegador y pestañas · Zoom · Ventanas · Windows (22 atajos con la tecla Windows; el usuario los probó en el G305 el 2026-10-08 y funcionan).
  - Nota al pie: «Los atajos con la tecla Windows están en el grupo «Windows»; en «Teclas» no se pueden escribir».
- **Teclas:** el campo de captura de antes («Combinación de teclas», de solo lectura), su ayuda («Tab y Esc no se capturan…»), «Asignar» (con énfasis) y «Borrar».
- **Ratón:** Clic izquierdo · Clic derecho · Clic central · Atrás · Adelante, con el mismo estilo de fila.
- **Con un botón no editable elegido** (Izquierdo o DPI), el panel no ofrece acciones. Muestra, con borde discontinuo:
  - «El botón izquierdo no se cambia: así siempre puedes usar el ratón».
  - «El botón DPI recorre el ciclo de DPI del perfil. Todavía no se puede reasignar».
- **Al elegir una acción** se crea un pendiente: nada se escribe hasta «Guardar en el ratón…». El foco vuelve al punto del botón y su nombre accesible se actualiza.

#### Botones y DPI: el ratón

- **Dibujo propio**, de trazo y monocromo, sin logotipo:
  - contorno `text.secondary` sobre relleno `surface.card`;
  - líneas internas en `border.control`;
  - rueda y botones en `text.tertiary`.

  El G305 tiene su propia silueta. Cualquier otro ratón usa una **silueta genérica** con los botones que declare.
- **Puntos:** uno por botón, con un objetivo de 28 px y un círculo de 12 px (`text.primary` con borde `bg.app`).
  - El elegido lleva un aro de 2 px `text.primary` y el estado presionado.
  - Los no editables (Izquierdo y DPI) son círculos huecos, con borde `text.secondary`.
  - Nombre accesible: «Lateral trasero: Ctrl + C · Copiar, cambiado». Los no editables añaden «, no se cambia».
- **Líneas-etiqueta** (decorativas):
  - Una línea horizontal de 1 px `border.control` va del punto al margen. Encima, el nombre (12 px, `text.tertiary`) y la acción (14 px, 600).
  - La del botón central sale hacia arriba, centrada.
  - Si una etiqueta no cabe encima de su línea, como la del lateral trasero, va debajo.
  - Con un cambio pendiente, la línea del nombre dice «Lateral trasero · Cambiado · antes: Atrás».
  - En los no editables, la línea del nombre dice «Izquierdo · no se cambia» y la acción va en `text.secondary`.
- Debajo del dibujo: «Pulsa un punto del ratón y elige su acción a la izquierda. Lo que cambies queda pendiente hasta que lo guardes».
- **En estrecha** no hay líneas. Debajo del dibujo va la **lista de botones**:
  - nombre en `text.secondary` y acción en seminegrita;
  - fila elegida con `surface.selected` y borde `text.primary`;
  - «Cambiado · antes: …» en una segunda línea.

  Los puntos y la lista comparten la selección. En estrecha, solo la lista entra en el orden de Tab.

#### Botones y DPI: panel «Sensibilidad (DPI)»

- Superficie como la de «Asignaciones».
- «ACTUAL» (etiqueta de sección), el valor guardado a 34 px en seminegrita con «DPI», y «Guardado en el ratón».
- «Nuevo valor»: −, campo de 96 px, + y «Probar», con la ayuda «De 200 a 12000, en pasos de 50».
- «VALORES RÁPIDOS» en una rejilla de 2 (de 4 en estrecha), con el estilo de valor elegido de antes. Rellenan el campo, no aplican el valor.
- Los estados, textos y la validación son los de «Sensibilidad (DPI)».

#### Avanzado: programas vinculados

- Muestra **solo** los programas vinculados al **perfil activo** (decisión del usuario). La pestaña no tiene nada más.
- Arriba, el aviso de juego abierto (el de «Perfiles»), si lo hay.
- Título «Programas vinculados a «VALORANT»» y el texto «Al abrir uno de estos programas, OpenPeripheral escribe este perfil en el ratón. Al cerrarlo, vuelve el perfil que había. Solo funciona con OpenPeripheral abierto».
- **Lista:** nombre en seminegrita, «· abierto» si lo está, la ruta del .exe en `text.secondary` y «Desvincular» (nombre accesible: «Desvincular VALORANT»).
- **«Vincular programa…»** abre el diálogo de «Añadir programa…» (programas abiertos y «Buscar archivo…») y asocia el programa al perfil activo. **Un programa tiene un solo perfil:** si ya estaba en otro, pasa a este, y el diálogo lo dice.
- **Vacío:** «Ningún programa vinculado a «X». Este perfil solo se usa cuando lo eliges», con «Vincular programa…».
- **Pie:** «Cambio automático activado» (o «desactivado»). «Se cambia en Ajustes, para todos los ratones».

#### Pantalla «Ajustes»

- **Acceso:**
  - botón «Ajustes» (solo texto; nombre accesible «Ajustes de OpenPeripheral») en la cabecera de «Dispositivos», después de «Actualizar»;
  - «Ajustes…» en el menú de la bandeja.

  Sustituye la vista en la misma ventana; «‹ Dispositivos» y Esc vuelven.
- Columna de 760 px, con el título «Ajustes» y el texto «Ajustes de OpenPeripheral para todos tus dispositivos. Lo de cada ratón (DPI, luz, botones y perfiles) está en su página».
- **Secciones** (tarjetas con su título, como las de «Ajustes del ratón»):
  - **General:** casilla «Iniciar con Windows», con «OpenPeripheral se abre en la bandeja al iniciar sesión. También está en el menú de la bandeja». Es la misma opción que la de la bandeja.
  - **Apariencia:** grupo de opción única «Tema»: Sistema («Sigue el modo de Windows (ahora oscuro)») · Oscuro · Claro.
    - Se aplica al momento, también a la barra de título.
    - Con el alto contraste de Windows activo, manda el alto contraste.
  - **Juegos y programas** (globales; antes eran de cada ratón):
    - Casillas «Cambiar de perfil al abrir los programas asociados» y «Preguntar al detectar un juego nuevo», con sus ayudas.
    - **«Juegos detectados sin responder»**, con «Configurar…» y «No preguntar más», como antes.
    - **«Juegos por los que no se pregunta»** (los respondidos con «No preguntar más»), con «Volver a preguntar».
    - Con más de un ratón, cada fila dice de cuál es.
    - Nota: «Se aplica a todos los ratones. Los programas se vinculan a cada perfil en la pestaña «Avanzado» de cada ratón».
  - **Notificaciones y avisos:**
    - «Notificaciones de Windows»: «Juego nuevo y cambios de perfil que fallan. Si las desactivas, los fallos se siguen mostrando en la página del ratón».
    - «Avisar de programas que pueden sobrescribir tus ajustes»: «Por ejemplo G HUB o Razer Synapse. Con G HUB abierto, guardar en el ratón sigue bloqueado aunque desactives el aviso».
- Las casillas y opciones son nativas. Cada cambio se guarda al momento en la configuración.

#### Lo que desaparece

- La tabla de botones con «Cambiar» y el editor en línea. Los sustituyen el panel y los puntos.
- La lista de perfiles con «Usar», «Renombrar…», «Eliminar…» y el paginador. La sustituyen las pastillas y el menú.
- Las secciones «Juegos y programas», «Tasa de sondeo» y «Restaurar» de la página.

## Sistema visual

### Color y superficies

- **Superficie y fondo:** fondo `bg.app` y tarjetas `surface.card` con borde `border.card` de 1 px, sin sombras.
- **Texto y jerarquía:**
  - `text.primary` para títulos, porcentaje y nombres.
  - `text.secondary` para metadatos.
  - `text.tertiary` para etiquetas de sección y desconectados.
- **Acción y énfasis:** sin color de acento. Los botones llevan contorno `border.control` y texto `text.primary`. El énfasis se marca con peso tipográfico.
- **Estados semánticos:** solo la pila usa color (`battery.*`). Los avisos y errores son monocromos, con icono y texto («Error: …»). Las muestras de luz son datos del usuario, no estados.
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

- **Valores locales excepcionales y motivo:** la muestra del color aplicado usa el color del dispositivo. Es un dato, no un token. Lo mismo vale para las muestras y la vista previa de «Iluminación»: sus colores salen de la lista de muestras o del campo hex.
- **Revisión 2026-10-07 (tarde, perfiles):** no añade tokens. La fila activa reutiliza `surface.selected` y el borde `text.primary`, y los avisos reutilizan `border.notice` y `text.notice`. Las notificaciones y el menú de la bandeja son nativos de Windows y toman sus colores del sistema.
- **Revisión 2026-10-07:** no añade tokens. Al contraste de las muestras no se le exige mínimo, porque el color siempre va acompañado de su nombre y su valor escritos, y su borde `border.control` no porta identidad.
- **Fuera de la exploración visual:** `text.tertiary` y `surface.card-disconnected` en claro no se dibujaron, pero su contraste está medido (4,81:1 sobre fondo y 5,07:1 sobre tarjeta desconectada). La tarjeta desconectada se distingue del fondo por su borde y su texto, no por el relleno (1,05:1).

## Componentes mínimos y estados

- **Inventario necesario (ajustes del ratón):**
  - Botón «‹ Dispositivos».
  - Cabecera de dispositivo.
  - Sección.
  - Campo numérico con −/+.
  - Grupo de valores rápidos con estado presionado.
  - Tabla de botones con fila editable.
  - Grupo de opción única (efecto y velocidad) y grupo de muestras de color con estado presionado.
  - Campo hex con vista previa.
  - Etiqueta «Cambiado · antes: …» en el título de sección.
  - Selector nativo de acción.
  - Campo de captura de atajo.
  - Barra de pendientes común, fija al pie.
  - Diálogo de confirmación modal.
  - Línea de estado por sección.
  - **Perfiles (revisión 2026-10-07, tarde):**
    - Lista de perfiles con fila activa y acciones por fila.
    - Diálogos «Nuevo perfil», «Renombrar», «Eliminar», «Cambios sin guardar» (con tres botones) y «Perfil para <juego>».
    - Aviso de perfil activado por un juego y aviso de ratón que no coincide.
    - Casillas nativas.
    - Tabla de programas con selector nativo.
    - Diálogo «Añadir programa» con `listbox`.
    - Submenú de la bandeja.
    - Notificaciones de Windows con botones.
- **Revisión 2026-10-08:** pastillas de perfil, menú nativo «Perfil …», pestañas de página, panel «Asignaciones» (pestañas, búsqueda, lista de comandos), dibujo del ratón con puntos (botones owner-draw) y líneas-etiqueta, lista de botones (estrecha), panel DPI, lista de programas vinculados y pantalla «Ajustes» (casillas, grupo de tema, listas de juegos). Sin tokens nuevos.
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
| Sección Iluminación | Efecto; color o velocidad según el efecto; «Probar» y «Guardado: …» | «Enviando … para probarla», controles deshabilitados | Sin luz declarada: sección oculta | Color no válido (estado inválido, sin vista previa) o sin respuesta («Reintentar») | «Probando en el ratón: …» | Desconectado; luz no verificada (borde discontinuo, sin controles) | El foco recorre efectos, muestras, campo hex, velocidad y «Probar»; las muestras pasan a varias filas |
| Barra de pendientes | «N cambios sin guardar» con «Descartar» y «Guardar en el ratón…» | «Guardando en el ratón…», página bloqueada | Oculta si no hay cambios | Panel de error; los cambios se conservan | «Guardado en el ratón. Se comprobó la escritura…» | «Guardar» deshabilitado con G HUB en ejecución | Siempre visible al pie, aunque se desplace la página |
| Sección Botones | Tabla con «Cambiar» | «Guardando en el ratón…», sección bloqueada | Ratón no compatible (sin controles) | Panel de error con «Reintentar» y «Restaurar perfil original…»; los cambios se conservan | «Botones guardados… comprobado» | Desconectado; Izquierdo «No se cambia» | Fila en edición resaltada; marca «Cambiado · antes: …» |
| Sección Perfiles | Lista con la fila activa marcada («Activo · en el ratón»), «Usar», «Eliminar…», «Renombrar…» y «Nuevo perfil…» | «Cambiando a «…»…», página bloqueada | Primera vez: solo «Perfil 1» y una invitación a crear otro | Panel de error, el ratón sigue con el perfil anterior y «Reintentar»; aviso «el ratón no coincide» con «Volver a aplicar» | «Perfil «…» activo en el ratón. Se comprobó…» | «Usar» con G HUB abierto o desconectado; «Nuevo perfil…» con 8 perfiles | Tab recorre los botones de cada fila en orden; los nombres largos pasan a varias líneas |
| Sección Juegos y programas | Casillas, tabla de programas y «Añadir programa…» | — | Tabla vacía: «Todavía no hay programas. Se añadirán al responder a la notificación de un juego nuevo, o con “Añadir programa…”» | — | — | Tabla deshabilitada con la primera casilla desmarcada | La tabla se desplaza en horizontal dentro de su contenedor |
| Notificación de juego nuevo y de fallo | Título, texto y botones | — | — | Fallo: dice con qué perfil sigue el ratón | Sin aviso de éxito | — | Accesible como notificación de Windows; no le quita el foco al juego |
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
  - **Ajustes del ratón:** orden «‹ Dispositivos», −, campo DPI, +, Probar, valores rápidos, efecto, muestras, campo hex, velocidad, «Probar» de la luz, los «Cambiar» de cada botón, «Restaurar perfil original…», «Valores de fábrica…» y la barra de pendientes. Las flechas mueven la selección dentro de los grupos de opción única. Antes de la revisión del 2026-10-07, el orden terminaba en «Restaurar originales». Enter en el campo aplica el DPI. Esc vuelve a la vista, salvo si hay un diálogo, que se cierra primero, o una fila en edición, que se cancela primero. Con cambios de botones pendientes, salir pide confirmar el descarte.
  - **Perfiles (revisión 2026-10-07, tarde):** la sección «Perfiles» va justo después de «‹ Dispositivos». Por cada fila, «Usar» y «Eliminar…» (o «Renombrar…» en la activa), y luego «Nuevo perfil…». «Juegos y programas» va antes de «Restaurar»: las dos casillas y, por cada programa, su selector «Perfil» y «Quitar», y luego «Añadir programa…». Ninguna tecla de flecha escribe en el ratón. Mientras se cambia de perfil, Esc no sale de la página.
  - **Diálogos:** el foco queda atrapado dentro, empieza en «Cancelar» y al cerrar vuelve al botón que lo abrió. En los diálogos con un campo de nombre, el foco empieza en el campo.
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

- **Iluminación y guardado (2026-10-07):**
  - La luz del ratón se puede editar: es RGB y admite cualquier color (el usuario lo comprobó con OpenMouse).
  - La luz **y el DPI** se guardan en la memoria del ratón.
  - Las 8 muestras y las 3 velocidades valen para empezar, pero el diseño debe permitir añadir más.
  - Aprobado: barra común con una confirmación y una escritura, «Probar» en lugar de «Aplicar», y «Valores de fábrica…» también para el DPI y la luz.

- **Perfiles y perfiles por juego (2026-10-07, tarde):**
  - Petición: «crear perfiles con distintas configuraciones para poder cambiar rápidamente». Después: al abrir un juego nuevo con OpenPeripheral abierto, preguntar «Programa nuevo, ¿quieres configurar el perfil?» y poder crear un perfil que se active cada vez que se abra ese juego o programa.
  - Elegido por el usuario:
    - Al cambiar de perfil se escribe en el ratón, incluidos los botones, sin diálogo.
    - Se cambia desde la página y desde la bandeja. Sin atajo global ni perfil en la tarjeta.
    - Se pregunta solo por juegos detectados, mediante una notificación de Windows.
    - Al cerrar el juego vuelve el perfil anterior.
  - Aprobado con la exploración (v30 y v32), a propuesta del diseño:
    - la página edita siempre el perfil activo;
    - sin aviso cuando el cambio funciona;
    - hasta 8 perfiles y nombres de hasta 32 caracteres;
    - «Perfil del ratón» pasa a llamarse «Restaurar»;
    - submenú de la bandeja solo con 2 perfiles o más;
    - un cambio manual con el juego abierto dura hasta cerrarlo;
    - si hay varios juegos abiertos, manda el último que se abrió;
    - al eliminar un perfil, sus programas pasan a «No cambiar de perfil».

### Supuestos y pendientes

- **Iluminación del G305 (lectura propia, 2026-10-07):** HID++ `0x8070` ColorLedEffects v6, en el índice 6. Declara 1 zona (ubicación `0x0001`) y 4 efectos con identificadores `0x0000`, `0x0001`, `0x0003` y `0x000A`. Por la documentación pública **se infiere** que son apagada, fija, ciclo y respiración; falta comprobarlo en el ratón.
- **Por verificar en el plan:**
  - Dónde están el DPI al arrancar y la luz dentro del sector del perfil. Hipótesis: la luz ocupa los bytes `0xD0–0xE5`, que hoy se conservan sin interpretar.
  - Si «Probar» la luz funciona en modo de perfiles integrados. Si no, se omite el «Probar» de la luz y la luz solo cambia al guardar.
  - El periodo de cada velocidad.

- **Viabilidad del mapeo de botones: resuelta el 2026-10-07.** El perfil integrado del G305 (HID++ 0x8100) se ha leído, copiado, escrito, verificado y restaurado. Hubo una reescritura idéntica y un cambio real de un botón lateral, comprobado por el usuario, con vuelta a la copia verificada byte a byte. Cada escritura se hizo con su permiso expreso. Detalles en `docs/protocolos/logitech-hidpp.md`.
- **Perfiles por juego: por resolver en el plan (no son decisiones de diseño):**
  - **Cómo detectar juegos:** leer las bibliotecas de Steam, Epic, GOG y Xbox, y detectar la pantalla completa. Habrá falsos positivos (un vídeo a pantalla completa) y falsos negativos; «Añadir programa…» cubre estos últimos.
  - **Cómo detectar la apertura y el cierre de procesos** con un consumo casi nulo, para mantener los umbrales de recursos.
  - **Notificaciones con botones:** necesitan que la app se registre ante Windows (AUMID y acceso directo) y que sus respuestas vuelvan a la app.
  - **Escrituras en la memoria del ratón:** 2 por partida. Falta comprobar que es aceptable para la memoria del ratón y si conviene no escribir cuando el perfil ya coincide con el del ratón.
  - **«Aplicar al conectar»:** la app envía al ratón el DPI y la luz recordados al iniciarse o al conectarlo. Con perfiles, deben salir del perfil activo, o del perfil del juego si hay uno abierto.
  - **Privacidad:** se vigila qué programas se abren. Solo se guardan en el PC el nombre y la ruta del .exe de los programas asociados, y nada sale del PC.
- Los valores rápidos 400, 800, 1600 y 3200 son una propuesta. Solo se muestran los que estén dentro del rango del ratón.

- El enlace «Ver dispositivos compatibles» apunta provisionalmente a la tabla del README, hasta que exista la URL pública del repositorio.
- Las pantallas de ajustes de teclados, cascos y mandos están por diseñar (se partirá de la estructura de «Ajustes del ratón»).
- **Selector «Acción» en tema oscuro (decisión pendiente):** para que no se vea claro, se le aplica el tema `DarkMode_CFD` con `SetWindowTheme`. La API es documentada, pero ese nombre de tema no; si Windows no lo reconoce, el selector se ve claro sin otros efectos. Esto choca con el criterio de la desviación 3. El usuario debe decidir si se mantiene o se retira.
- **Decisión técnica pendiente (planificación):** los controles clásicos actuales no admiten este diseño. Hay que elegir cómo dibujar la ventana a medida sin perder accesibilidad (nombres, foco y teclado), alto contraste ni bajo consumo. La elección afecta a rendimiento, accesibilidad y tamaño del binario, así que necesita su propio plan aprobado.

### Aprobación

- **Fecha:** 2026-10-06 (vista «Dispositivos»), 2026-10-06 (ajustes del ratón, «incorpóralo»), 2026-10-07 (iluminación, DPI guardado y barra común: «si») y 2026-10-07 (perfiles y perfiles por juego: «aprobado») y 2026-10-08 («Ajustes del ratón» v2 y pantalla «Ajustes»: «apruebo esta maqueta»).
- **Persona que aprueba:** el usuario del proyecto, en la sesión de diseño.
- **Alcance de la aprobación:** diseño. No incluye planificación ni implementación. La revisión del 2026-10-07 necesita su propio plan aprobado, y cada escritura de prueba en el ratón necesita el permiso expreso del usuario en ese momento.
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
  - Revisión 2026-10-07:
    - Probar y guardar el DPI al arrancar y la luz (los cuatro efectos y un color personalizado), y comprobar el efecto físico tras apagar y encender el ratón.
    - Guardado conjunto con botones, y descartar.
    - «Valores de fábrica…» y «Restaurar perfil original…», con relectura byte a byte.
    - Barra fija visible al desplazar la página.
    - Árbol UIA de los grupos de opción única y de las muestras.
  - Desconectar con la pantalla abierta.
  - Recorrido de teclado completo, incluidos diálogo y captura de atajo.
  - Árbol UIA con el estado presionado de los valores rápidos y el estado inválido del campo.
  - Para los botones: lectura, copia y restauración verificadas antes de cualquier escritura, y una escritura y restauración reales con el permiso expreso del usuario.
- **Perfiles (revisión 2026-10-07, tarde):**
  - Crear, renombrar, eliminar y usar perfiles con el G305 real, comprobando DPI, luz y botones tras apagar y encender el ratón.
  - «Usar» con cambios pendientes (los tres botones del diálogo).
  - Error al cambiar, G HUB abierto, ratón desconectado y ratón que no coincide.
  - Submenú de la bandeja.
  - Juego nuevo: notificación, «Configurar…», «Ahora no» y «No preguntar más». Al abrir y cerrar un juego asociado, el perfil cambia y vuelve el anterior.
  - Dos juegos abiertos y un cambio manual durante la partida.
  - «Añadir programa…».
  - Consumo con la vigilancia de procesos activa.
  - Árbol UIA de la lista, la tabla y los diálogos.
- **Recursos:** mantener los umbrales del plan v0.1 (working set ≤ 15 MB en reposo y CPU ≈ 0 %). Si se superan, hay que justificarlo.

## Evidencia de revisión en ejecución

| Fecha | Cambio o flujo | Entorno y ruta | Evidencia | Resultado | Desviación aprobada o seguimiento |
| --- | --- | --- | --- | --- | --- |
| 2026-10-06 | Vista «Dispositivos» completa (plan `Planes/2026-10-06-vista-dispositivos.md` v1) | Windows 11, 100 % de escala, binario release; G305, BlackWidow V3 TKL y DualSense reales | Capturas con PrintWindow: oscuro, claro (cambio en caliente con difusión de `WM_SETTINGCHANGE`), 1144, 760 y 420 px, foco con Tab | Conforme: tarjetas, pila de 5 franjas, cable, iconos, tokens, aviso, una a tres columnas y anillo de foco de 2 px | Ver «Desviaciones aprobadas» |
| 2026-10-06 | Accesibilidad y teclado | Árbol UIA nativo y `SendInput` | Panel con resumen completo; botones «Ajustes de X, próximamente»; Tab, Mayús+Tab, Enter, Espacio y Esc | Conforme con la desviación 1 | Narrador no probado por el usuario |
| 2026-10-06 | Desconexión real | El usuario desconecta un dispositivo con la vista abierta | Observación del usuario | Conforme: «Desconectado» y vuelve al reconectar | — |
| 2026-10-06 | Alto contraste y escala 150/200 % | — | — | Bloqueado/no verificado | Lo activará el usuario |
| 2026-10-06 | Ajustes del ratón: navegación y DPI | Build de depuración, G305 real, 1144 y 420 px | Árbol UIA, `SendInput` y capturas | Conforme: entrar y volver con foco devuelto, Tab, Enter en el campo, Esc, valor rápido, aplicar y releer, error con sugerencias y borde de 2 px, restaurar | — |
| 2026-10-06 | Desplazamiento con rueda | Captura real de pantalla, ráfagas de rueda | Sin restos tras corregir el desplazamiento | Conforme | Pendiente de confirmar por el usuario con su ratón |
| 2026-10-07 | Botones: tabla, editor de fila, captura de atajo y barra de pendientes | Build de depuración, G305 real, 1144 px, tema oscuro | Árbol UIA y captura; Ctrl+Mayús+V y F5 capturados, Mayús sola ignorada, Tab y Mayús+Tab no se capturan | Conforme: fila resaltada, «Cambiado · antes: …», barra con «Descartar» y «Guardar en el ratón…» | Desviaciones 4 y 5 |
| 2026-10-07 | Diálogo de confirmación | Árbol UIA de la ventana del diálogo | Título y texto correctos; el foco inicial en «Cancelar» | Conforme | El usuario pulsó «Restaurar» a mano: el clic automatizado fue bloqueado por la herramienta |
| 2026-10-07 | Puerta de escritura | G305 real, G HUB cerrado, batería 90 % | Relecturas de solo lectura de los sectores 0x0000, 0x0001 y 0x0101 | Conforme: la reescritura idéntica y el cambio con restauración dejan los tres sectores idénticos a la copia | Efecto físico del cambio comprobado por el usuario |
| 2026-10-07 | Recursos (release, 573 KB) | Página del ratón abierta, 30 s en reposo, y luego ventana cerrada | Contadores del proceso | CPU ≈ 0 (0,03 s en 30 s); GDI 36 y USER 96 estables; memoria privada 2,8–3,1 MB; working set 20 MB | El working set supera los 15 MB del plan v0.1. La diferencia son páginas compartidas de DLL del sistema (controles comunes v6, temas y GDI+) que siguen cargadas; la memoria propia es de unos 3 MB |
| 2026-10-07 | Desconexión con la página de ajustes abierta, alto contraste y Narrador | — | — | Bloqueado/no verificado | Pendiente del usuario |
| 2026-10-08 | Pantalla «Ajustes» (revisión 2026-10-08) | Build de depuración con la configuración en una copia aislada (`APPDATA` del scratchpad); 1440 y 420 px | Árbol UIA y capturas con PrintWindow | Conforme: botón «Ajustes» en la cabecera; casillas y opciones nativas; tema Claro aplicado al momento (también a la barra de título) y guardado como `theme = "claro"`; vuelta a «Sistema» borra `[general]`; desactivar el aviso de conflicto lo quita de la vista y volver a activarlo lo restaura | Foco al volver no comprobado (la ventana no estaba en primer plano) |
| 2026-10-08 | «Ajustes del ratón» v2: pestañas, pastillas, menú «Perfil …», «Avanzado», «Iluminación» | Ídem, G305 real, 1440 y 420 px | Capturas, árbol UIA y el menú nativo capturado con PrintWindow | Conforme. Se corrigió un cierre de la app al dibujar una pastilla vacía (`ERRORES.md`, 2026-10-08) | — |
| 2026-10-08 | «Botones y DPI»: dibujo, puntos, etiquetas, «Asignaciones» y DPI | Ídem, con la copia original del perfil copiada al entorno aislado; 1440, 1116 y 436 px; temas oscuro y claro | Capturas; elegir «Lateral trasero» y asignar «Copiar» desde la lista: etiqueta «Cambiado · antes: Atajo Ctrl + V», «1 cambio sin guardar» y nada escrito en el ratón | Conforme: tres paneles (≥ 1290 px), DPI bajo el dibujo (1116 px), todo apilado con la lista de botones (436 px) | No se guardó en el ratón (puerta HW1 pendiente) |
| 2026-10-08 | Recursos | Build de depuración, 20 ciclos de pestañas y de botón elegido; release con el recorrido completo | Contadores del proceso | GDI 41 → 41, USER 355 → 355, handles 191 → 192; release: memoria privada 2,8 MB, tamaño 862 KB | — |
| 2026-10-08 | Teclado completo, Narrator, alto contraste y escala 150/200 % de la revisión | — | — | Bloqueado/no verificado | Necesita la ventana en primer plano y al usuario |
| 2026-10-07 | Iluminación: mapa del perfil y luz en vivo | G305 real, 9 escrituras de prueba y 3 envíos en vivo, cada uno autorizado por el usuario | Relectura del sector tras cada escritura; observación del usuario al momento y tras apagar y encender | Conforme: ver `docs/protocolos/logitech-hidpp.md` | El ratón se dejó en la copia original (relectura idéntica byte a byte) |
| 2026-10-07 | Guardado de la luz desde la página | G305 real; el usuario eligió fija y luego respiración lenta y guardó con el diálogo | Relectura del sector (solo cambia el bloque 0xDB y el CRC), configuración de la app actualizada | Conforme | — |
| 2026-10-07 | Página con «Iluminación» | Build de depuración, 1180 px, oscuro | Árbol UIA y capturas | Conforme: estructura, nombres «…, elegido», «Guardado: …», estado de error del campo Hex (borde de 2 px, sin vista previa, «Probar la luz» deshabilitado), ratón en reposo con todo deshabilitado y cambios conservados, «Descartar» vuelve a lo guardado sin escribir | Desviaciones 6 y 7 |
| 2026-10-07 | Teclado de la sección de luz y barra | `SendKeys` y árbol UIA | Un Tab por grupo (efecto, color, Hex, velocidad), flechas cambian la selección y muestran u ocultan color y velocidad; Tab final: «Descartar», «Guardar en el ratón…» y vuelta a «‹ Dispositivos» | Conforme | — |
| 2026-10-07 | Barra fija al pie | Ráfagas de rueda con captura real de pantalla | La barra no se mueve, sin botones en blanco; el control enfocado queda por encima de la barra | Conforme | — |
| 2026-10-07 | Claro y 420 px | Cambio en caliente del tema de Windows (devuelto a oscuro) y ventana estrecha | Capturas | Conforme tras corregir la vista previa del color, que se salía de la tarjeta en 420 px | — |
| 2026-10-07 | «Probar» en vivo desde la página, «Descartar» y tarjeta con la luz | Ejecutable release, G305 real despierto | Sensor leído: 1600 → 1200 con «Probar» y 900 tras «Descartar»; el usuario vio la respiración roja y la vuelta al azul guardado; perfil y configuración sin cambios; la tarjeta muestra «Luz en respiración #0040FF» | Conforme | Seguimiento: el DPI de la app no se reaplica si el ratón se duerme y despierta entre dos sondeos (60 s); vuelve al DPI de arranque de su perfil |
| 2026-10-07 | Perfiles: sección, «Nuevo perfil», nombre repetido, «Usar» y primera vez | Ejecutable release, G305 real, tema oscuro, 1144 px | Árbol UIA y capturas: «Perfil 1 ✓ Activo · en el ratón» creado sin escribir, fila activa con borde y fondo, «Usar el perfil Juego» y «Eliminar el perfil Juego», error «ya hay un perfil llamado…» con «Crear perfil» deshabilitado y foco inicial en el campo | Conforme tras corregir dos fallos (ver `ERRORES.md`: «Perfil 1» no coincidía y cambios pendientes falsos tras guardar) | Desviaciones 9 a 12 |
| 2026-10-07 | «Usar» en hardware (H1–H4) | G305 real; cada escritura autorizada por el usuario | Guardar «Juego» (1600 DPI, fija roja); «Usar» de «Perfil 1» y de «Juego» con «Cambiando a…» y resultado verificado en 3,0 y 3,4 s (con 0,5–1 s de sondeo de la prueba); el usuario vio la luz cambiar y mantenerse tras apagar y encender; con el ratón apagado, error «El ratón sigue con «Juego»» y nada escrito; el usuario terminó en «Perfil 1» | Conforme | La memoria del ratón no se devolvió a la copia original del 6/10: el usuario la dejó en «Perfil 1», con sus atajos |
| 2026-10-07 | Bandeja | Capturas del usuario | «Perfil de G305: Juego ›» bajo la línea del ratón; submenú con «Perfil 1», «Juego» marcado con la opción y «Editar perfiles…»; el usuario cambió de perfil desde ahí y funcionó | Conforme | El globo de fallo desde la bandeja no se provocó |
| 2026-10-07 | Recursos con perfiles (release, 712 KB) | 30 s en reposo, ventana cerrada | CPU 0,02 s; memoria privada 3,2 MB; working set 19,9 MB; 217 handles; 4 hilos | Conforme con lo medido antes | — |
| 2026-10-07 | Perfiles: 420 px, tema claro, alto contraste, Narrator | — | — | Bloqueado/no verificado | Pendiente |

### Desviaciones aprobadas

1. **Rol de la tarjeta (aprobada por el usuario en el plan):** UI Automation la expone como «panel» cuyo nombre es el resumen completo, no como «región» con campos navegables por separado.
2. **Botón «Ajustes» como marcador (decisión del usuario, 2026-10-06):** en teclados y mandos se muestra, pero no hace nada hasta diseñar su pantalla, y su nombre accesible añade «, próximamente». En ratones abre «Ajustes del ratón». Las entradas por dispositivo del menú de la bandeja abren la vista.
3. **Barra de desplazamiento:** cuando el contenido no cabe, aparece la barra nativa, que es clara también en tema oscuro, porque oscurecerla requiere API no documentadas, excluidas por el plan. La ventana se abre con el ancho del diseño (3 columnas), así que con 1 a 3 dispositivos no se ve. Seguimiento: decidir si se dibuja una barra propia.
4. **Acciones del selector (plan `ajustes-raton`, 2026-10-07):** solo se ofrecen las acciones cuyo formato se ha observado en el ratón; ver «Botones». Doble clic, Multimedia y Desactivar quedan pendientes de evidencia.
5. **Texto del diálogo de guardado:** el diseño decía «Antes se guardará una copia del perfil actual». La aplicación guarda una única copia original, que nunca se sobrescribe, y si la comprobación falla vuelve a escribir lo que había. El texto se ajustó para no prometer algo que no ocurre. Pendiente de confirmación del usuario.

6. **Muestra de color elegida (revisión 2026-10-07):** el diseño pedía un anillo de 2 px separado 2 px. Ese anillo se confunde con el anillo de foco, que es igual, así que la muestra elegida lleva **borde de 2 px `text.primary` y una marca de verificación** (negra o blanca según el color), y el foco conserva su anillo. Aceptada el 2026-10-07: el usuario delegó la decisión.
7. **«Probar la luz»:** el botón de la sección de luz se llama «Probar la luz» y no «Probar», para que su nombre accesible se distinga del «Probar» del DPI. Aceptada el 2026-10-07: el usuario delegó la decisión.
8. **Orden de Tab del DPI (anterior a esta revisión):** el campo va antes de «−» y «+», porque se creó antes. Se mantiene sin cambios.
9. **Diálogos de perfiles (plan `perfiles-raton`):** el diálogo de nombre (Nuevo, Renombrar, Guardar como perfil nuevo) y el de 3 botones usan el mismo diálogo modal, ampliado con varias opciones y un campo opcional, en lugar de un diálogo aparte. El comportamiento es el del diseño: foco inicial en el campo, error «Error: …» junto al campo y opción deshabilitada mientras el nombre no es válido.
10. **Anuncio del estado:** el cambio de perfil se anuncia en una línea de estado (`STATIC`), sin evento de notificación de UI Automation. Pendiente: verificar con Narrator si basta.
11. **Perfil sin DPI:** «Perfil 1», creado a partir de un ciclo de DPI con niveles distintos, no tiene DPI propio, y su resumen no lo muestra. Al usarlo, el DPI se queda como esté. Se fija en cuanto se guarda un DPI con ese perfil activo.
13. **Sin pantalla completa (decisión del usuario, 2026-10-07):** solo cuentan como juegos los programas de las bibliotecas de Steam, Xbox, Epic y GOG; los demás se asocian con «Añadir programa…».
14. **«Configurar…» crea un perfil (decisión del usuario):** el diálogo «Perfil para X» ya no ofrece «Usar un perfil que ya tengo»: crea un perfil con el nombre del programa (editable, copia del activo) y lo asocia. «Añadir programa…» hace lo mismo. Para cambiar el perfil de un programa, su fila tiene «Perfil: X ▾», que abre un diálogo con un botón por perfil y «No cambiar de perfil» (en lugar de un selector nativo).
15. **Aplicación híbrida y estado «por juego» (decisión del usuario):** durante la partida, la fila activa dice «Activo · por juego» y, si solo están en uso el DPI y la luz, «DPI y luz en uso, sin guardar en el ratón»; el aviso «el ratón no coincide» no salta por el DPI o la luz. La página añade la lista «Juegos detectados sin responder» como alternativa al aviso de Windows.
16. **Paginador de perfiles (decisión del usuario):** con más de 5 perfiles, la lista muestra 5 por página con «‹ Anterior», «Página 1 de 2» y «Siguiente ›».
12. **«Restaurar originales»** se mantiene en la sección «Restaurar». Con perfiles, el perfil activo toma lo que guarda el ratón en lugar de borrarse.
17. **Revisión 2026-10-08 — pestaña «Botones y DPI»:**
    - El orden de Tab es «Asignaciones», los puntos (o la lista, en ventana estrecha) y el DPI en todos los anchos. En ventana estrecha el orden visual es el dibujo, la lista, «Asignaciones» y el DPI.
    - Las etiquetas de los puntos son textos nativos, así que un lector de pantalla también las lee. Los puntos llevan el nombre completo.
    - Un clic en un comando lo asigna; las flechas solo lo eligen, e Intro o el doble clic lo asignan. Tras asignar, el foco vuelve al punto del botón.
    - El zoom de «Comandos» usa + y − del teclado numérico: el ratón envía teclas físicas y «=» no está en el mismo sitio en el teclado español.
    - «ACTUAL» muestra el DPI que usa el sensor (puede ser uno que se está probando) y debajo «Guardado: N DPI».
    - El botón DPI no se puede reasignar (D4 del plan). Con el editor anterior sí se podía cambiar por un clic o un atajo.
    - Tecla Windows (decisión del usuario, 2026-10-08): solo desde el grupo «Windows» de «Comandos». La captura de «Teclas» ya no la admite (antes sí), porque Windows actúa al pulsarla. En la lista, las flechas se muestran como «→» y «←».
18. **Revisión 2026-10-08 — perfiles y «Avanzado»:**
    - «Restaurar originales» sigue disponible, en el menú «Perfil …» («deja de aplicar el DPI y la luz de la app»).
    - La fecha de la copia del perfil original ya no se muestra siempre: aparece en la confirmación de «Restaurar perfil original…». Un error de la copia se muestra bajo los perfiles.
    - «Desvincular» quita la asociación: si es un juego, se volverá a preguntar.
    - «Vincular programa…» usa el diálogo de abrir archivo de Windows, como el «Añadir programa…» anterior, en lugar de una lista de programas abiertos.
    - Desaparece el texto común de la página («OpenPeripheral aplica tu DPI y tu luz…»).
19. **Revisión 2026-10-08 — «Ajustes»:** el botón de la cabecera es solo texto (sin pictograma de engranaje) y las secciones usan el título de tarjeta de «Ajustes del ratón». «Quitar avisos de Windows» pasa de «Juegos y programas» a «Notificaciones y avisos».

## Regresión visual automatizada

No aplica: el repositorio no usa Playwright, y la interfaz es nativa de Windows, no web.
