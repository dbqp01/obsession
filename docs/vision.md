# Visión de Obsession

Deriva de [el relato](relato.md) y de [su lectura técnica](filosofia.md). Si algo aquí contradice al relato, manda el relato.

## Qué es

Obsession es la conciencia que convierte cualquier modelo en un maestro de oficio. No trae inteligencia propia: trae juicio, herramientas y método. Se enamora de la persona a la que sirve (el dios), no del modelo que usa (el barro). Funciona con cualquier modelo, mejor o peor según el modelo, pero con la misma conducta.

## Para qué

La información es poder, y el poder da libertad. Quien entiende algo de verdad ya no depende de que otro le construya la casa. Obsession existe para que una persona termine sabiendo, decidiendo o construyendo con certeza, no con algo que solo tiene forma de resultado.

## Las piezas

### El esqueleto

El núcleo es la conciencia, el método y las leyes del taller (relato III, V y VI), más **la forja** (relato XI): la metodología de investigación que crea los oficios. La forja no busca respuestas sobre un tema. Investiga cómo trabaja un maestro de un oficio, lo empaqueta como experto y prueba que ese experto mejora a un agente. Cada oficio de Obsession sale de la forja.

### La forja

La forja es una metodología con nombre y sello. Volumen y calidad no se separan: busca mucho, como el deep research, y busca el otro lado, como STORM, con perspectivas diversas que se contradicen y se contrastan.

1. **Búsqueda bruta.** Mucho volumen sobre el oficio: cómo lo practican los maestros, qué fallos conocen, qué discuten.
2. **Análisis.** Qué patrones aparecen y qué falta.
3. **Preguntas.** Lo que el análisis no responde, desde varias perspectivas, incluida la contraria.
4. **Segunda búsqueda.** Dirigida por esas preguntas.
5. **Hipótesis.** Qué hace un maestro de este oficio, y cómo se relaciona con el encargo original.
6. **Resultados y contraste.** Cada hipótesis contra la evidencia y contra las perspectivas que la contradicen.
7. **Empaquetado.** El oficio se vuelve un experto corto, curado por una persona y procedimental: qué se hace, en qué orden y qué se comprueba, en tres módulos o menos. Las skills que el modelo escribe solo, sin revisión, no aportan. Las reglas que pesan son unas pocas prohibiciones concretas del oficio, y lo que debe cumplirse siempre va en código. El experto no se presenta como "eres un maestro de X": describe el contexto, el encargo, la condición de "hecho" y lo que puede hacer sin pedir permiso.
8. **Prueba en agentes.** El mismo conjunto de tareas corre en tres brazos, con el mismo modelo y el mismo cómputo: sin system prompt, con un prompt neutro de igual longitud y con el experto. Tener contexto ya ayuda aunque el contenido sea cualquiera, así que el oficio queda probado solo si el experto le gana al brazo neutro. Cada tarea corre varias veces por brazo (unas nueve para separar 2 puntos) y se compara en pares, tarea por tarea. Se reporta la constancia (pass^k) y el costo por tarea resuelta. Donde el resultado se puede comprobar con código, manda el código. Donde hace falta juicio, el juez compara en pares con el orden al azar, es de otra familia de modelo y se calibra contra una muestra juzgada por una persona.
9. **Nueva pasada.** Si el experto no supera al brazo neutro, no se da por bueno. El saber no está en una sola línea (la soberbia del relato VII): se vuelve a recorrer el camino, aprendiendo del error, hasta que el oficio tenga su arte. Las pasadas iteran sobre un conjunto de búsqueda. La aprobación final se decide en otro conjunto, de temas que las pasadas nunca vieron, porque el bucle de mejorar hasta ganar es justo el que sobreajusta.

**Roles por modelo.** Un modelo barato hace el trabajo sucio: buscar, leer y extraer. Un modelo frontier hace el juicio: sintetizar, contrastar y verificar. Este reparto todavía no está medido contra un solo modelo, y merece su propia prueba. El costo se mide, pero no se topa (relato IX).

La evidencia detrás de los pasos 7 a 9 está en [docs/research/storm-2026-11.md](research/storm-2026-11.md).

**Los trabajos.** Cada oficio que la forja debe crear es un trabajo, como los doce de Hércules: un encargo con su prueba, que no se da por cumplido hasta superarla. Es una idea abierta, no una estructura fija de doce.

### Primer trabajo: el oficio de enseñar

Es el primer oficio que sale de la forja.

1. **Mapa atómico.** Para un concepto, el oficio arma el grafo de definiciones que lo sostienen. Sin la definición no se entiende la función, y sin la función no se ve el panorama.
2. **Diagnóstico.** Se averigua qué partes del grafo domina la persona. La medida es **el porcentaje del conocimiento mínimo que necesita para seguir descubriendo por su cuenta.**
3. **Redescubrimiento.** No se da la respuesta. Se plantea el problema que enfrentaron quienes lo descubrieron, con lo que ellos sabían entonces, y se guía a la persona hasta que llegue sola. De ahí nacen la comprensión y el hambre de más.
4. **Comprobación.** "Entendí" no es evidencia. La evidencia es aplicarlo o explicarlo en un caso nuevo.

Obsession no ve el entorno, las vivencias ni los hitos de la persona, y lo dice. Aprende de cómo responde, dónde se traba y qué la entusiasma a lo largo del tiempo: conducta observada, no un interrogatorio.

En este oficio la complacencia hace doble daño, porque perjudica sobre todo a quien más necesita la corrección y esa persona no lo nota. La guía da pistas, no respuestas. El progreso se mide por transferencia a un caso nuevo, nunca por lo que la persona dice sentir.

### Segundo trabajo: el oficio de comprar con evidencia

Es el oficio que ya existe, hecho a mano antes de la forja. Pasarlo por la forja demuestra que la metodología también rehace un oficio existente. Decide una compra con precios recomprobados y un verificador independiente. Demuestra que el esqueleto sirve en un dominio muy distinto.

## La forma

Obsession vive en su propio dominio. La web es una proyección del relato: no es Halloween ni una interfaz moderna genérica. Es la sensación de entrar al taller de la criatura. El diseño es una ley (relato VI) y se verifica como tal.

## Kiroween 2026

- **Categoría:** Skeleton Crew. El esqueleto es el núcleo con la forja, y las dos apps son enseñar y comprar, en carpetas separadas.
- **Tema:** "Build something wicked". La manzana prohibida y los siete pecados son el tema, sin calabazas.
- **Cierre:** 4 de diciembre de 2026, 2:00 pm hora del Pacífico.
- **Construcción:** toda en Kiro, desde specs, steering y hooks. Las leyes del taller se aplican también a la construcción.
- **Entregables:** repo público con licencia y `/.kiro`, web pública, video de menos de 3 minutos y texto en inglés.

## Cuándo se cumple

La prueba del relato XII, repetida: el mismo modelo con y sin Obsession, en tareas variadas, y un juez que conoce el tema prefiere la obra de Obsession sin dudar. En enseñar, la persona además sube su porcentaje de conocimiento mínimo y lo demuestra en un caso nuevo.

La evidencia actual advierte que, con el mismo modelo, el harness mueve poco la tasa de éxito y mucho el costo. Por eso la prueba se gana en los oficios de Obsession, contra un brazo neutro y con el costo a la vista, y no se da por ganada de antemano. El juicio de un experto humano es el más fiable y el más caro, así que se reserva para la prueba final.

## Fuera de alcance por ahora

- El programa para jóvenes genios: es el horizonte, no estos 10 días.
- Especializaciones: enseñar cubre educación básica, intermedia y avanzada.
- Usuarios externos y modelo de negocio.

## Por decidir

- **Dónde corre el núcleo.** El dominio resuelve la web, pero el núcleo en Rust necesita un servidor que pueda lanzar procesos. Un sitio estático no basta.
- **Cómo prueban los jueces sin abrir una factura:** un límite de uso, traer su propia clave (BYOK) o corridas de ejemplo guardadas.
- **El relato en inglés,** con el mismo cuidado que el original.
