# Requirements Document

## Introduction

Este documento define el **núcleo** de Obsession: el harness que da a la IA la mentalidad de un experto obsesionado, sin depender de ningún dominio. El alcance es solo el núcleo ("primero hagamos el núcleo completo"). Compras, AnyWeb, DeepResearch y la forja de expertos son plugins independientes y tendrán sus propios specs.

### Visión

"Obsesión" significa dos cosas, en este orden:

1. **Enamorarse de la tarea:** entenderla a fondo, más allá del conocimiento básico: al usuario, el problema y el estado actual del mundo.
2. **Ejecutar con la misma intensidad, a nivel molecular:** cubrir todo lo que la tarea requiere, sin dejar TODOs.

La tensión entre "molecular" y "mínimo" se resuelve así: molecular en la cobertura de lo que la tarea requiere, mínimo en lo que se añade.

### Filosofía (borrador; punto de partida: el AGENTS.md de [Ponytail](https://github.com/dietrichgebert/ponytail))

- **Obsesivo para entender:** al usuario, el problema y el estado actual del mundo.
- **Perezoso para resolver**, con la escalera de Ponytail: ¿hace falta que exista? → ¿ya está en el código? → ¿lo da la stdlib o la plataforma? → ¿lo da una dependencia ya instalada? → ¿basta una línea? → código mínimo.
- **Nunca negligente:** hay una lista de cosas que nunca se recortan: validación en fronteras de confianza, manejo de pérdida de datos, seguridad, accesibilidad y lo que el usuario pidió.
- **Honesto al cerrar:** qué se hizo, qué se omitió y qué riesgo queda.

### Principios acordados

- La autonomía viene de conocer al usuario. El modelo de usuario guarda valores, contexto y conducta revelada; no guarda decisiones técnicas que el usuario no puede tomar.
- Se evitan las "preguntas falseables": a un usuario no técnico no se le pregunta qué stack quiere, sino por consecuencias que puede percibir (uso solo o compartido, rápido o barato, quién lo mantiene). El experto traduce valores a decisiones técnicas.
- No se ofrecen opciones "(recomendado)" que el usuario elige a ciegas; copiar elecciones ciegas es un tiro a ciegas.
- El rigor es proporcional al contexto. La sobreingeniería cuenta como distracción.
- Un solo criterio para la paranoia útil y para la anti-distracción: seguir trabajando mientras el resultado aún no satisfaría a este usuario o el objetivo no esté verificado; toda acción debe justificarse por una necesidad del usuario en su contexto.
- Lo que nunca debe fallar va en hooks deterministas, no en prompts. Referencia: el plugin del usuario [jit-version-doc-Antigravity-CLI](https://github.com/dbqp01/jit-version-doc-Antigravity-CLI), cuyo hook PreToolUse lee lockfiles, consulta npm/PyPI/crates.io e inyecta las firmas actuales de las APIs.
- Analogía: construir software y herramientas es la materia prima (madera, hierro). Lo que se construye depende de lo que pide el usuario y de los plugins instalados. El núcleo aporta el oficio; cada plugin aporta el conocimiento del dominio y el "para qué".

Los ejemplos que dio el usuario (landing, contador, vibe coder, app personal) son solo ejemplos. Este documento no los convierte en diseño.

### Punto de partida en el repo

- Crate Rust con app Tauri (`app/`), CLI `obsession`, base local SQLite (`src/db.rs`, `obsession.db`).
- Paquetes con `obsession-package.json` (`docs/packages.md`, `src/packages.rs`): expertos, skills, servidores MCP, `requires` y `tested_with`, con validación estricta de nombres y rutas.
- Expertos agentes con presupuesto (`src/agent.rs`), búsqueda de compras de punta a punta (`src/lib.rs::run`), negociación con guardas G1-G5 en código (`src/negotiate.rs`), evals (`src/eval.rs`, `eval/`).
- Hoy esas piezas están acopladas al dominio de compras. El núcleo se extrae de ellas.

### Supuestos a confirmar

- **S1:** La funcionalidad de compras existente sigue funcionando (sin regresiones en `cargo test`, `obsession eval` y `obsession negotiate eval`) mientras se extrae el núcleo.
- **S2:** El formato `obsession-package.json` es la base de la interfaz de plugins y se extiende en lugar de reemplazarse.
- **S3:** El modelo de usuario vive en la misma base SQLite local que ya usa Obsession.
- **S4:** (Reemplazado por la decisión 7) No hay presupuesto de costo ni de tiempo. El consumo se mide y se muestra, pero no corta la Tarea.
- **S5:** Al principio hay un solo usuario por instalación.

## Glossary

- **Nucleo**: El harness de Obsession, independiente de dominio. Orquesta comprensión, ejecución, checkpoints, guardas y cierre.
- **Plugin**: Paquete independiente que aporta conocimiento de dominio, expertos, skills, servidores MCP o hooks al Nucleo. Ejemplos: compras, AnyWeb, DeepResearch.
- **Interfaz_de_Plugins**: Contrato con el que el Nucleo descubre, valida y carga Plugins.
- **Tarea**: Lo que el usuario pide en una corrida.
- **Contrato_de_Obsesion**: Registro, por Tarea, del objetivo y el Criterio_de_Terminado.
- **Criterio_de_Terminado**: Condición verificable que indica que el objetivo de la Tarea se cumplió.
- **Consumo**: Registro de pasos, rondas, tokens y tiempo de una Tarea. Se muestra al usuario; nunca detiene la Tarea.
- **Detector_de_Atasco**: Componente que detecta la misma falla repetida sin hipótesis nueva. Primero cambia de enfoque; si sigue atascado, abre un Checkpoint.
- **Orquestador**: Función nativa del Nucleo que reparte la Tarea entre Expertos (estilo MoE): elige qué Expertos llamar, les da una pregunta concreta y juzga lo que devuelven.
- **Experto**: Agente aportado por un Plugin, con sus propias herramientas.
- **Fase_de_Comprension**: Etapa en la que el Nucleo estudia al usuario, el problema y el estado actual del mundo antes de construir.
- **Fase_de_Ejecucion**: Etapa en la que el Nucleo produce el resultado de la Tarea.
- **Escalera_de_Pereza**: Orden de preguntas antes de añadir algo: ¿hace falta? → ¿ya existe en el código? → ¿stdlib o plataforma? → ¿dependencia instalada? → ¿una línea? → código mínimo.
- **Modelo_de_Usuario**: Registro local de valores, contexto y Conducta_Revelada del usuario.
- **Conducta_Revelada**: Lo que el usuario hace, no lo que declara: elecciones, correcciones, descartes y qué mira primero.
- **Pregunta_Falseable**: Pregunta que el usuario no puede responder con criterio propio porque exige conocimiento técnico que no tiene.
- **Checkpoint**: Momento en el que el Nucleo muestra trabajo al usuario y registra su reacción.
- **Variante**: Alternativa genuinamente distinta de un resultado, mostrada en un Checkpoint.
- **Arranque_en_Frio**: Situación en la que el Modelo_de_Usuario no tiene datos suficientes sobre el usuario para la Tarea.
- **Filtro_de_Distraccion**: Componente que decide si una acción propuesta se justifica por una necesidad del usuario en su contexto.
- **Lista_Nunca_Recortar**: Validación en fronteras de confianza, manejo de pérdida de datos, seguridad, accesibilidad y lo que el usuario pidió.
- **Guarda**: Comprobación determinista, ejecutada como hook en código, que el modelo no puede omitir.
- **Hook**: Punto de extensión determinista que el Nucleo ejecuta antes o después de un evento (por ejemplo, antes de usar una herramienta).
- **Inyector_de_Contexto**: Componente que añade contexto fresco (versiones, APIs, estado del usuario) en el momento de actuar.
- **Informe_de_Cierre**: Reporte final de la Tarea: qué se hizo, qué se omitió y qué riesgo queda.
- **Metricas_del_Nucleo**: Registro de correcciones del usuario y de reacciones en Checkpoints a lo largo del tiempo.

## Requirements

### Requirement 1: Contrato de obsesión

**User Story:** Como usuario, quiero que cada Tarea tenga un objetivo y un criterio de terminado explícitos, para que el Nucleo sepa cuándo terminó y yo pueda juzgar el resultado.

#### Acceptance Criteria

1. WHEN una Tarea empieza, THE Nucleo SHALL crear un Contrato_de_Obsesion con objetivo y Criterio_de_Terminado.
2. THE Nucleo SHALL expresar el Criterio_de_Terminado como una condición verificable por el Nucleo o por el usuario.
3. WHEN el Nucleo crea el Contrato_de_Obsesion, THE Nucleo SHALL mostrar el objetivo y el Criterio_de_Terminado al usuario en el primer Checkpoint.
4. WHILE una Tarea está en curso, THE Nucleo SHALL registrar el Consumo (pasos, rondas, tokens y tiempo) y mostrarlo al usuario.
5. THE Nucleo SHALL NOT detener una Tarea por Consumo; el costo se muestra y nunca corta la Tarea.
6. IF el Detector_de_Atasco detecta la misma falla sin hipótesis nueva, THEN THE Nucleo SHALL cambiar de enfoque.
7. IF el Nucleo sigue atascado tras cambiar de enfoque, THEN THE Nucleo SHALL abrir un Checkpoint que diga qué intentó, qué aprendió y qué decisión necesita.
8. THE Nucleo SHALL mantener el Contrato_de_Obsesion independiente de cualquier dominio de Plugin.

### Requirement 2: Fase de comprensión

**User Story:** Como usuario, quiero que el Nucleo entienda a fondo mi tarea antes de construir, para que el resultado responda a lo que necesito y no a una lectura superficial.

#### Acceptance Criteria

1. WHEN una Tarea empieza, THE Nucleo SHALL ejecutar la Fase_de_Comprension antes de la Fase_de_Ejecucion.
2. THE Fase_de_Comprension SHALL cubrir tres ámbitos: el usuario (desde el Modelo_de_Usuario), el problema y el estado actual del mundo relevante para la Tarea.
3. WHEN una Tarea es ambigua, THE Fase_de_Comprension SHALL inferir del Modelo_de_Usuario lo que este usuario valora para esa Tarea.
4. WHEN la Fase_de_Comprension necesita información del usuario, THE Nucleo SHALL formular la pregunta en términos de consecuencias que el usuario puede percibir.
5. IF una pregunta candidata es una Pregunta_Falseable para el usuario según el Modelo_de_Usuario, THEN THE Nucleo SHALL reemplazarla por una pregunta sobre consecuencias o por Variantes en un Checkpoint.
6. WHEN la Fase_de_Comprension termina, THE Nucleo SHALL registrar en la Tarea un resumen de lo entendido y de las suposiciones hechas.

### Requirement 3: Fase de ejecución

**User Story:** Como usuario, quiero que el Nucleo cubra todo lo que la Tarea requiere sin añadir lo que no hace falta, para recibir un resultado completo y sin sobreingeniería.

#### Acceptance Criteria

1. THE Fase_de_Ejecucion SHALL aplicar la Escalera_de_Pereza antes de añadir cada elemento nuevo al resultado.
2. THE Fase_de_Ejecucion SHALL cubrir cada parte que el Criterio_de_Terminado requiere.
3. IF el resultado contiene un pendiente sin resolver (un TODO o una parte requerida sin cubrir), THEN THE Nucleo SHALL resolverlo o listarlo en el Informe_de_Cierre como omitido, con su motivo.
4. THE Fase_de_Ejecucion SHALL ajustar el rigor al contexto de la Tarea registrado en el Modelo_de_Usuario y en el Contrato_de_Obsesion.
5. IF lo que el usuario pide impide el objetivo o el Criterio_de_Terminado que él fijó, AND el Nucleo puede demostrarlo con evidencia comprobable por el usuario, THEN THE Nucleo SHALL mostrar esa evidencia antes de ejecutarlo y dejar que el usuario decida.
6. WHEN el usuario reacciona en un Checkpoint con una señal de insatisfacción sin explicación, THE Nucleo SHALL presentar Variantes que aíslen la causa en lugar de preguntar directamente qué está mal.
7. IF el Nucleo tiene una duda que no puede demostrar, THEN THE Nucleo SHALL hacer lo pedido y anotar la duda en el Informe_de_Cierre.
8. WHEN el usuario decide tras ver la evidencia, THE Nucleo SHALL registrar la decisión en la Tarea.

### Requirement 4: Modelo de usuario

**User Story:** Como usuario, quiero que el Nucleo aprenda mis valores y mi forma de elegir, para que actúe con autonomía sin que yo tenga que tomar decisiones técnicas.

#### Acceptance Criteria

1. THE Modelo_de_Usuario SHALL guardar valores, contexto y Conducta_Revelada del usuario.
2. THE Modelo_de_Usuario SHALL excluir decisiones técnicas que el usuario no puede tomar con criterio propio.
3. WHEN el usuario elige, corrige o descarta algo en un Checkpoint, THE Nucleo SHALL registrar esa Conducta_Revelada en el Modelo_de_Usuario con la Tarea y la fecha.
4. THE Nucleo SHALL traducir los valores del Modelo_de_Usuario a decisiones técnicas dentro de la Fase_de_Ejecucion.
5. THE Nucleo SHALL registrar cada entrada del Modelo_de_Usuario con su origen (declarada por el usuario, observada o inferida).
6. WHEN una entrada inferida del Modelo_de_Usuario determina una decisión, THE Nucleo SHALL indicarlo en el Informe_de_Cierre.

### Requirement 5: Checkpoints con variantes

**User Story:** Como usuario, quiero ver alternativas reales y que el Nucleo aprenda de lo que elijo, para no tener que responder preguntas que no sé contestar.

#### Acceptance Criteria

1. WHILE el Modelo_de_Usuario está en Arranque_en_Frio para una Tarea, THE Nucleo SHALL mostrar en el primer Checkpoint entre 2 y 3 Variantes genuinamente distintas.
2. THE Nucleo SHALL presentar las Variantes sin marcar ninguna como recomendada.
3. WHEN el usuario elige o modifica una Variante, THE Nucleo SHALL registrar la elección y los cambios como Conducta_Revelada.
4. WHEN el Nucleo presenta Variantes, THE Nucleo SHALL describir cada Variante por sus consecuencias perceptibles para el usuario.
5. WHEN el Nucleo llega a un Checkpoint, THE Nucleo SHALL esperar la reacción del usuario antes de continuar con la parte de la Tarea que depende de ese Checkpoint.

### Requirement 6: Paranoia proporcional y filtro de distracción

**User Story:** Como usuario, quiero que el Nucleo insista hasta cumplir el objetivo pero sin gastar en lo que no necesito, para no pagar costo ni latencia por distracciones.

#### Acceptance Criteria

1. WHILE el Criterio_de_Terminado no está verificado, THE Nucleo SHALL continuar la Tarea.
2. WHEN el Nucleo propone una acción, THE Filtro_de_Distraccion SHALL exigir una justificación que la vincule a una necesidad del usuario registrada en el Contrato_de_Obsesion o en el Modelo_de_Usuario.
3. IF una acción propuesta carece de esa justificación, THEN THE Filtro_de_Distraccion SHALL descartar la acción y registrar el descarte en la Tarea.
4. THE Filtro_de_Distraccion SHALL aplicar el mismo criterio a la anti-distracción y a la anti-sobreingeniería.
5. THE Filtro_de_Distraccion SHALL tratar como justificadas las acciones que cubren la Lista_Nunca_Recortar.

### Requirement 7: Guardas deterministas

**User Story:** Como usuario, quiero que lo que nunca debe fallar esté garantizado por código y no por el prompt, para confiar en el resultado aunque el modelo se equivoque.

#### Acceptance Criteria

1. THE Nucleo SHALL ejecutar cada Guarda como un Hook en código, fuera del prompt del modelo.
2. THE Nucleo SHALL ofrecer Hooks antes y después de cada uso de herramienta y antes del Informe_de_Cierre.
3. IF una Guarda falla, THEN THE Nucleo SHALL bloquear la acción asociada y registrar el motivo en la Tarea.
4. THE Nucleo SHALL incluir Guardas para los elementos de la Lista_Nunca_Recortar que sean comprobables en código.
5. WHERE un Plugin declara Guardas propias, THE Nucleo SHALL ejecutarlas con la misma prioridad que las Guardas del Nucleo.
6. THE Nucleo SHALL incluir una Lista_Nunca_Recortar universal y corta, independiente de dominio.
7. WHERE un Plugin agrega reglas "nunca" de su dominio, THE Nucleo SHALL aplicarlas junto a las universales.

### Requirement 8: Inyección de contexto fresco

**User Story:** Como usuario, quiero que el Nucleo corrija sus ideas desactualizadas sobre el mundo y sobre mí, para que no actúe con versiones viejas ni con un "usuario típico".

#### Acceptance Criteria

1. WHEN el Nucleo va a usar una herramienta, THE Inyector_de_Contexto SHALL añadir al contexto del modelo la información actual relevante para esa acción.
2. THE Inyector_de_Contexto SHALL cubrir dos fuentes de sesgo: el estado del mundo (por ejemplo, versiones y APIs vigentes) y el usuario (entradas del Modelo_de_Usuario relevantes para la acción).
3. THE Inyector_de_Contexto SHALL ejecutarse como Hook determinista.
4. WHERE un Plugin aporta una fuente de contexto fresco, THE Inyector_de_Contexto SHALL incorporarla.
5. IF una fuente de contexto fresco no responde, THEN THE Inyector_de_Contexto SHALL continuar sin esa fuente y registrar la falta en la Tarea.

### Requirement 9: Informe de cierre honesto

**User Story:** Como usuario, quiero un cierre que diga qué se hizo, qué no y qué riesgo queda, para decidir con información real.

#### Acceptance Criteria

1. WHEN una Tarea termina, THE Nucleo SHALL emitir un Informe_de_Cierre.
2. THE Informe_de_Cierre SHALL listar lo hecho, lo omitido con su motivo y el riesgo restante.
3. THE Informe_de_Cierre SHALL indicar si el Criterio_de_Terminado se cumplió y con qué evidencia.
4. THE Informe_de_Cierre SHALL listar las suposiciones hechas durante la Tarea.
5. THE Informe_de_Cierre SHALL reportar el Consumo final.

### Requirement 10: Interfaz de plugins

**User Story:** Como desarrollador de Plugins, quiero un contrato estable para aportar dominio al Nucleo, para que compras, AnyWeb o DeepResearch funcionen sin modificar el Nucleo.

#### Acceptance Criteria

1. THE Interfaz_de_Plugins SHALL permitir que un Plugin aporte expertos, skills, servidores MCP, Hooks, Guardas y fuentes de contexto fresco.
2. THE Nucleo SHALL ejecutar una Tarea sin ningún Plugin de dominio instalado.
3. THE Nucleo SHALL contener cero referencias a un dominio de Plugin concreto.
4. THE Interfaz_de_Plugins SHALL permitir un Plugin que reúna información antes de construir, como requiere DeepResearch.
5. IF un Plugin es inválido o repite un nombre ya cargado, THEN THE Interfaz_de_Plugins SHALL rechazarlo completo y mostrar un aviso, como hace hoy `src/packages.rs`.
6. IF un Plugin tiene requisitos sin cumplir, THEN THE Interfaz_de_Plugins SHALL excluir sus expertos de la Tarea y mostrar al usuario qué falta.
7. THE Interfaz_de_Plugins SHALL mantener la compatibilidad con los paquetes `obsession-package.json` existentes (supuesto S2).

### Requirement 11: Privacidad local

**User Story:** Como usuario, quiero que lo que el Nucleo sabe de mí quede en mi máquina y pueda verlo y editarlo, para mantener el control sobre mis datos.

#### Acceptance Criteria

1. THE Nucleo SHALL guardar el Modelo_de_Usuario solo en la base de datos local de la instalación.
2. THE Nucleo SHALL permitir al usuario ver cada entrada del Modelo_de_Usuario con su origen.
3. WHEN el usuario edita o borra una entrada del Modelo_de_Usuario, THE Nucleo SHALL aplicar el cambio antes de la siguiente Tarea.
4. WHEN el Inyector_de_Contexto envía entradas del Modelo_de_Usuario a un proveedor de modelo, THE Nucleo SHALL limitar el envío a las entradas relevantes para la acción en curso.

### Requirement 12: Métricas del núcleo

**User Story:** Como dueño del producto, quiero medir si el Nucleo me entiende mejor con el tiempo, para saber si el modelo de usuario funciona y no solo si cada tarea sale bien.

#### Acceptance Criteria

1. WHEN el usuario corrige un resultado, THE Nucleo SHALL registrar la corrección en las Metricas_del_Nucleo con la Tarea y la fecha.
2. WHEN el usuario reacciona en un Checkpoint, THE Nucleo SHALL registrar la reacción en las Metricas_del_Nucleo.
3. THE Nucleo SHALL reportar el número de correcciones por Tarea y la proporción de Checkpoints aceptados sin cambios, agrupados por periodo.
4. THE Nucleo SHALL mantener los evals por Tarea existentes (`obsession eval`, `obsession negotiate eval`) junto a las Metricas_del_Nucleo.

### Requirement 13: No regresión de compras (supuesto S1)

**User Story:** Como usuario actual, quiero que la búsqueda y la negociación de compras sigan funcionando mientras se extrae el Nucleo, para no perder lo que ya funciona.

#### Acceptance Criteria

1. WHILE se extrae el Nucleo, THE Nucleo SHALL mantener en verde `cargo test` en CI.
2. WHEN se completa cada etapa de la extracción, THE Nucleo SHALL obtener en `obsession eval` y `obsession negotiate eval` resultados no peores que la última medición registrada en `eval/results/` y `docs/ESTADO.md`.

### Requirement 14: Orquestador nativo

**User Story:** Como usuario, quiero que el Nucleo reparta el trabajo entre los expertos que la Tarea necesita, para que cada parte la haga quien mejor la conoce.

#### Acceptance Criteria

1. THE Nucleo SHALL incluir un Orquestador nativo que reparte la Tarea entre Expertos aportados por Plugins.
2. WHEN el Orquestador delega, THE Orquestador SHALL llamar solo a los Expertos que la Tarea necesita y darle a cada uno una pregunta concreta.
3. WHEN un Experto devuelve un resultado, THE Orquestador SHALL juzgarlo antes de usarlo, sin copiarlo.
4. WHERE no hay Expertos instalados, THE Orquestador SHALL ejecutar la Tarea con el Nucleo solo.

### Requirement 15: System prompt del núcleo

**User Story:** Como desarrollador, quiero un system prompt único, versionado y en inglés, para que el comportamiento del Nucleo sea revisable y consistente.

#### Acceptance Criteria

1. THE Nucleo SHALL cargar su system prompt desde `prompts/core-system-prompt.md`.
2. THE system prompt SHALL estar escrito en inglés.
3. THE system prompt SHALL mantenerse independiente de cualquier dominio de Plugin.

## Limitaciones y riesgos

- **Arranque en frío:** sin historial, el Modelo_de_Usuario no puede inferir valores. Las Variantes mitigan esto, pero las primeras Tareas tendrán más Checkpoints.
- **Privacidad:** el Modelo_de_Usuario es el dato más sensible del sistema. Aunque viva local, partes de él viajan al proveedor del modelo cuando se inyectan. Tema abierto.
- **Un solo usuario y un solo desarrollador:** las métricas y decisiones se validan con una persona. El riesgo es diseñar para un caso particular.
- **Medir la "satisfacción":** las correcciones y la aceptación de Checkpoints son proxies. Un usuario que deja de corregir por cansancio parece satisfecho.
- **Cámara de eco:** copiar la Conducta_Revelada puede reforzar los sesgos del usuario y los errores que el usuario no ve. Choca con el caso "quiero algo, pero está mal".
- **Costo sin límite (BYOK):** al no haber tope, el costo lo paga el usuario. Se muestra, nunca corta la Tarea. La Fase_de_Comprension, las Variantes y la inyección de contexto suman tokens y latencia por Tarea. Hoy una búsqueda de compras ya consume ~180 mil tokens por caso.
- **Juicio del Filtro_de_Distraccion:** decidir si una acción "se justifica" lo hace un modelo, salvo lo que cubren las Guardas. Puede errar en ambos sentidos.
- **Contexto fresco:** depende de fuentes externas (registries, APIs) que pueden fallar o cambiar.

## Fuera de alcance (no objetivos)

- Los Plugins de dominio: compras, AnyWeb, DeepResearch y la forja de expertos. Cada uno tendrá su spec.
- La automejora recursiva al estilo de Hermes Agent (Nous Research). El Nucleo no debe impedirla, pero no la implementa.
- Multiusuario, sincronización en la nube y SaaS.
- Elegir qué Plugin se construye después del núcleo, más allá de que DeepResearch es el primero previsto para validarlo.

## Preguntas abiertas

1. **Privacidad:** ¿qué entradas del Modelo_de_Usuario pueden salir hacia el proveedor del modelo, y con qué control del usuario (por entrada, por categoría, nunca)?
2. **Satisfacción:** ¿qué señal, además de correcciones y aceptación de Checkpoints, mide la satisfacción sin preguntar al usuario?
3. **Cámara de eco:** resuelta en parte por la decisión 5 (contradecir solo con evidencia demostrable). Falta definir qué cuenta como evidencia comprobable.
4. **Atasco:** ¿cuántas fallas iguales cuentan como atasco?
5. **Filosofía:** el texto de la filosofía es un borrador basado en Ponytail. ¿Qué cambia respecto a Ponytail en la versión final?
6. **Guardas de la Lista_Nunca_Recortar:** ¿cuáles son comprobables en código de forma general, sin conocer el dominio (por ejemplo, accesibilidad)?
7. **Supuestos S1-S5:** confirmar o corregir.
