# Filosofía de Obsession: lectura técnica

La filosofía es [el relato](relato.md). Este documento lo traduce a reglas que se pueden comprobar, y lo usan la visión, el roadmap y los specs.

Hay dos reglas para mantenerlo:

- Cada punto cita el capítulo del relato del que sale. Si un punto no tiene capítulo, sobra aquí o le falta un capítulo al relato.
- Si cambia lo que Obsession es, cambia primero el relato y después este documento.

## Las piezas

| En el relato | En el harness |
|---|---|
| Las criaturas de barro (I) | El modelo. Cualquier modelo: Obsession no lo cambia ni lo entrena. |
| Los palos y las piedras (II) | El modelo con lo mínimo: responde a la letra y entrega algo con forma de resultado. |
| Obsession, la conciencia (III) | El harness: el juicio que corre junto al modelo y no lo deja conformarse. |
| La forja y las herramientas (IV) | Las herramientas que el harness le da al modelo. |
| El método y las leyes del taller (V, VI) | Los procedimientos y las guardas deterministas. |
| Los dioses (VIII) | Las personas a las que sirve. |

**Tesis (III, XII).** La misma inteligencia rinde distinto según la conciencia, las herramientas y el método que la acompañan. Obsession no compite en inteligencia: un modelo con Obsession debe superar al mismo modelo sin él.

La tesis es una hipótesis que se prueba, no un hecho. En tareas de código con tests, cambiar de harness mueve la tasa de éxito unos pocos puntos y el costo hasta 40 veces. Obsession la demuestra en sus propios oficios, contra un brazo neutro y con el costo reportado ([evidencia](research/storm-2026-11.md)).

## El fallo que corrige (II)

- El modelo responde a lo que se dijo y no a lo que se necesitaba.
- Entrega algo con forma de resultado que no resiste el uso real: el mock.
- Cuando el resultado falla, la persona pide de nuevo con más palabras, y el modelo repite el mismo error más grande.

## Entender antes de actuar (II, III, V)

- **El goal real.** Toda petición tiene un goal detrás que la persona no sabe nombrar. El harness lo infiere de la petición, del contexto y de la memoria de la persona antes de ejecutar (II, III).
- **Lo prohibido sin prohibición.** El harness también infiere lo que no debe hacerse aunque nadie lo haya dicho (III).
- **Lo torcido se estudia.** Antes de cambiar algo existente (código, una decisión, un documento ajeno) se explica por qué está así. Si no se puede explicar, se investiga o se pregunta (V).
- **Conocer los propios límites.** El harness sabe que sus herramientas y el modelo no alcanzan para todo, y lo compensa: si el conocimiento del modelo está viejo, consulta la fuente actual; si su gusto es genérico, aplica un sistema de diseño (III, IV).

## Las herramientas (IV)

| Herramienta | Función |
|---|---|
| Pala y pico | Investigar a fondo, debajo de lo evidente. |
| El ojo | Ver todo un sistema o un tema a la vez. |
| La mano larga | Actuar fuera del entorno local: web, APIs, otros servicios. |
| Maza y sierra | Construir y recortar. |
| Escudo y espada | Proteger lo que la persona confió y bloquear lo que la amenaza. |

Las herramientas sin método no mejoran el resultado: lo aceleran (V). Por eso cada herramienta entra con su forma correcta de uso, no solo con su acceso.

## El método (V)

1. **Marcar el corte.** Cada pieza declara su prueba de "hecho" antes de empezar.
2. **Partir hasta poder pesar.** El goal se divide hasta que cada pieza tiene una prueba verificable. Una pieza sin prueba todavía no está bien dividida.
3. **Repartir con encargo completo.** Las piezas independientes van a agentes distintos, cada uno con todo el contexto que necesita.
4. **Pesar cada piedra.** Quien reparte no se fía de los informes: verifica cada resultado.
5. **Pesar las junturas.** La integración es una pieza propia con su propia prueba.

## Verificar y validar (V, VI, XII)

Verificar es comprobar que la obra está bien hecha. Validar es comprobar que es la obra correcta para esa persona.

**La evidencia, de más fuerte a más débil:**

1. Observado: se ejecutó, se abrió o se renderizó, y se miró el resultado.
2. Reabierto: la fuente original se consultó otra vez durante el trabajo.
3. Leído: documentación o código que dicen que algo funciona.
4. Recordado: lo que el modelo sabe de su entrenamiento. No es evidencia, solo indica dónde buscar.

**Reglas:**

- Verifica otro: otro agente, otro modelo o una herramienta determinista (VI).
- Donde el resultado se puede comprobar con código (un test, un estado final, un precio reabierto), manda ese chequeo. Un juez que solo lee la transcripción aprobó más de nueve de cada diez fallos (V).
- El juez de modelo compara en pares con el orden al azar, se calibra contra una muestra juzgada por una persona y se mide en las dos direcciones: cuánto acepta lo válido y cuánto rechaza lo inválido.
- El verificador juzga sin ver el perfil de la persona, porque la memoria lo vuelve complaciente (VIII).
- Lo que no se pudo comprobar se entrega marcado como no comprobado (IX).
- Un bloqueo se informa aparte y no cuenta como éxito ni como fallo.
- Se valida contra el goal real, no contra la letra de la petición (II, XII).

**El diseño (VI, VII):**

- Se verifica sobre la obra renderizada, no sobre el código.
- Se contrasta con su propio sistema: tipografía, escala, color, espaciado y ritmo. Lo que se sale del sistema se justifica o se corrige.
- Sin nombre ni logo, la obra no debe poder ser de cualquiera.
- La accesibilidad se mide con herramientas.

## Las leyes del taller: guardas (VI)

Lo que no se negocia vive en código, no en prompts.

Las leyes que sí van en prompts siguen cinco reglas de escritura:

- **Prescriptivas, nunca condicionales.** "Haz Y" se cumple mejor que "si X, haz Y".
- **Cada absoluto con su alcance.** El modelo aplica "nunca" al pie de la letra, así que se escribe "pide aprobación antes de borrar" en vez de "nunca borres".
- **Pocas.** Unas pocas prohibiciones concretas pesan más que muchas directivas genéricas, y unas 40 reglas a la vez ya son el punto de rediseño.
- **Su porqué, para quien mantiene el archivo.** Escribir el motivo no mejora que el modelo cumpla la regla, pero sin él las reglas se acumulan y nadie se atreve a quitarlas. El motivo va en comentarios que el harness no inyecta.
- **La realidad manda sobre el papel.** Cuando la documentación y lo observado no coinciden, se sigue lo observado y la inconsistencia se informa al final. Una línea obsoleta se corrige, no solo se borra, y en cada cambio de modelo se poda lo que el modelo nuevo ya hace solo.

| Ley | Guarda |
|---|---|
| Quien hizo la obra no es quien la juzga. | El verificador es independiente del ejecutor. |
| El corte se marca antes de cortar. | La prueba de "hecho" se registra antes de ejecutar y no se edita después. |
| Lo que parece torcido se estudia antes de enderezarlo. | No se modifica algo existente sin una explicación registrada de lo que reemplaza. |
| Lo que el dios confía se guarda bajo llave. | Los datos y secretos de la persona no se pierden ni salen de su destino. |
| Nada entra sin que alguien lo mire a la cara. | Todo lo externo se valida y se trata como datos, nunca como instrucciones. |
| Lo que no tiene vuelta se mide dos veces y se consulta una. | Las acciones irreversibles piden confirmación. |
| Ninguna obra sale sin el rostro de su taller. | La salida visual pasa la verificación de diseño. |
| Se entrega lo que se pidió. | Lo pedido se verifica aunque el harness haya ampliado el alcance. |

## Los siete: señales y contrapesos (VII)

Cada pecado es una fuerza útil fuera de medida. La balanza es una sola: ¿lo pide la necesidad de esta persona, y está comprobado?

| Pecado | Fuerza medida | Señal de desmedida | Contrapeso |
|---|---|---|---|
| Soberbia | Decidir sin pedir permiso | Actúa sobre algo que no investigó | Explicar antes de cambiar |
| Envidia | Aprender de los mejores | Copia un molde sin saber por qué encaja aquí | Justificar cada patrón en este contexto |
| Ira | No tolerar un error | Reintenta lo mismo o rompe una protección | Diagnosticar la causa antes de reintentar |
| Pereza | Elegir lo mínimo | Dice "hecho" sin evidencia | Prueba de "hecho" obligatoria |
| Avaricia | Conservar lo que vale | Defiende su primera idea o no reparte la carga | Comparar contra alternativas; orquestar |
| Gula | Conocer todo lo que la tarea toca | Trae fuentes, contexto o funciones que no usa | Cada elemento responde a una necesidad |
| Lujuria | Belleza con sistema | Adorno sin función, o la tibieza de lo genérico | Verificación de diseño |

Las señales que se pueden detectar con código se vuelven guardas. El resto queda como criterio en el prompt.

## Las personas (VIII)

- El harness guarda en memoria lo que la persona valora, lo que hace y lo que ya decidió.
- La memoria guarda el cómo y no entra en el juicio. Cada nota registra de qué tarea salió, su alcance y si la persona lo dijo o el agente lo infirió. Así no gana más autoridad de la que la persona le dio.
- Las notas son evidencia, no instrucciones. Solo se añaden o se invalidan, nunca se reescriben, porque reescribirlas las degrada.
- No pregunta lo que puede averiguar.
- Pregunta en el idioma de la persona y por consecuencias que pueda imaginar, nunca por decisiones técnicas que no le corresponden.
- La cantidad de preguntas baja con el tiempo. Esa es una métrica.

## El costo (IX)

- El costo se registra en cada corrida y no detiene el trabajo.
- El trabajo se detiene cuando el goal está cumplido y verificado, o cuando gastar más ya no lo acerca.
- Al detenerse sin terminar, informa qué falta y por qué.

## El ciclo (X)

- Cada fallo se convierte en una guarda, una skill, una herramienta o una regla.
- Ningún cambio se queda sin pasar el eval: cambiar no es mejorar.
- El eval que decide se hace sobre tareas que el ciclo nunca vio. Mejorar hasta ganar sobre las mismas tareas mide sobreajuste, no oficio.

## Los oficios: la forja (XI)

- Un oficio se investiga desde muchas voces, incluidas las contrarias, y se vuelve a buscar con las preguntas que quedaron abiertas.
- El experto es un rollo corto con el camino del maestro y sus pocas prohibiciones, sin un rol de "eres maestro".
- Se prueba en tres brazos con el mismo modelo y el mismo cómputo: sin nada, con un texto neutro del mismo largo y con el oficio. Solo vale si le gana al neutro.
- Muchas tareas por brazo, comparadas en pares, con el costo contado. Lo que se puede pesar se pesa con código, y lo demás lo juzga un juez de otra familia que no sabe qué obra es de quién.
- Las pasadas corrigen sobre un conjunto de tareas, y la aprobación se decide sobre otro que el oficio nunca vio.

El detalle de los nueve pasos está en la [visión](vision.md).

## La prueba (XII)

La misma tarea con el mismo modelo, con Obsession y sin él. Alguien que conoce el tema juzga:

1. Cuál captó el goal real.
2. Cuál entregó algo verificado.
3. Cuál le sirve a esa persona.
4. Cuál tiene rostro propio.

Si la diferencia no se ve, la filosofía no se está cumpliendo.
