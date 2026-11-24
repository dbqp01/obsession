# Requirements Document

## Introduction

Este documento define los requisitos de la hoja de ruta de Obsession para los próximos 12 meses. El rumbo elegido por el dueño es demostrar que Obsession es un harness general: la obsesión acotada (buscar y verificar hasta tener evidencia, dentro de un presupuesto de pasos, rondas y tokens) debe servir para un dominio distinto de compras antes de buscar usuarios externos. El acompañante y el SaaS quedan fuera de este horizonte.

Punto de partida, según `docs/ESTADO.md` (2026-11-20):

- Compras en Perú funciona y está medido: `obsession eval` da 10/10 casos sin error, 9/10 en menos de 5 min, 1 pick falso de 47 y unos 180 mil tokens por caso. Hay 12 fuentes. La negociación copiloto da 0 violaciones en el simulador. La app Tauri y el CI en Windows funcionan.
- Falta el juicio humano del eval de compras (meta ≥ 7/10), las reglas R1-R10 del dueño, la clave de afiliado de AliExpress y la aprobación de Hiraoka en AnyWeb.
- La app lee `packages/` del repo donde se compiló, así que no es portable a otra PC.
- Las skills de `packages/core/skills/` (`price-normalization`, `negotiation`, `counterfeit-signals`, `report-findings`, `stop-criteria`) y los prompts de `prompts/` (`planner.md`, `ranker.md`, `verifier.md`, `expert_harness.md`) mezclan la lógica general del harness con la de compras.
- El watch (`docs/design/watch.md`) está diseñado y sin código. El panel de negociación (PR 4) y el modo autónomo (PR 5) de `docs/design/negotiation.md` están pendientes.

La hoja de ruta tiene cinco fases. La Fase 0 cierra lo mínimo de compras. La Fase 1 separa el núcleo del harness de lo específico de compras. La Fase 2 construye el segundo dominio, elegir stack de software, comparando librerías y herramientas con evidencia de documentación, benchmarks e issues. La Fase 3 valida ese dominio con un eval propio. La Fase 4 decide, con los datos de las fases anteriores, si se abre el trabajo de acompañante y SaaS.

### Propuesta de cierre mínimo de compras (Fase 0)

El dueño pidió cerrar 1 o 2 piezas clave antes del segundo dominio, sin elegirlas. Esta hoja de ruta propone dos:

1. **Juicio humano del eval de compras.** Sin ese juicio, "la mejor compra" no está medida: el eval solo mide errores, tiempo y picks falsos. El segundo dominio copiará el mismo método de eval, así que hace falta saber primero si el método produce un criterio útil. Cuesta unas horas del dueño y ningún código.
2. **Empaquetar `packages/` como recursos de la app.** El segundo dominio llegará como paquete. Si los paquetes dependen del repo de compilación, ningún dominio nuevo es portable, y la Fase 1 tendría que resolverlo igual. Es un cambio acotado en la app y en la carga de paquetes.

Quedan fuera de la Fase 0, con su motivo:

- **Watch (PR 1-2).** Es una feature nueva de 2 PRs con programador de tareas y toasts. No aporta a la generalidad del harness y su diseño ya está escrito, así que puede retomarse después sin pérdida.
- **Hiraoka (`review` y `package` en AnyWeb).** Es barato y depende solo de la aprobación humana, pero suma una fuente número 13 a un dominio que ya tiene 12. Se hace si el dueño tiene tiempo; no bloquea ninguna fase.
- **Conector de AliExpress por API.** Depende de una clave que todavía no existe.
- **Panel de negociación (PR 4) y tono menos repetitivo.** Mejoran compras, no la generalidad.

## Glossary

- **Hoja_de_Ruta**: el plan de 12 meses descrito en este documento, con sus fases, puertas, métricas, límites y no-objetivos. Vive en `docs/` del repo.
- **Fase**: bloque de trabajo de la Hoja_de_Ruta con objetivo, entregables, métricas de salida y duración estimada.
- **Puerta_de_Fase**: conjunto de métricas de salida que una Fase debe cumplir antes de que empiece la Fase siguiente.
- **Dueño**: la persona que desarrolla Obsession y toma las decisiones humanas (juicio de eval, aprobaciones, claves, reglas).
- **Harness**: el programa Obsession completo (CLI `obsession`, núcleo en `src/lib.rs`, app Tauri en `app/`).
- **Nucleo**: la parte del Harness que no depende de un dominio: planificador, expertos agentes, recomprobación de evidencia, verificador de otra familia de modelo, ranker con rondas, presupuestos de pasos, rondas y tokens, paquetes, MCP, proveedores y eval.
- **Dominio**: área de decisión que el Harness atiende con un paquete propio, por ejemplo compras en Perú o elegir stack de software.
- **Paquete_de_Dominio**: paquete con formato `obsession-package.json` (ver `docs/packages.md`) que aporta los expertos, skills, prompts específicos, fuentes y casos de eval de un Dominio.
- **Dominio_Compras**: el Dominio actual, compras en Perú, con 12 fuentes y la negociación copiloto.
- **Dominio_Stack**: el segundo Dominio, elegir stack de software, que compara librerías y herramientas con evidencia de documentación oficial, benchmarks publicados, issues y actividad de repositorios.
- **Evidencia**: un dato con fuente (URL https) que el Harness volvió a abrir y comprobó durante la corrida.
- **Afirmacion_Falsa**: en Dominio_Stack, una afirmación del veredicto que no coincide con su Evidencia al reabrir la fuente, o que no tiene fuente. Es el equivalente del pick falso de Dominio_Compras.
- **Eval_de_Dominio**: el conjunto fijo de casos de un Dominio y el comando que los corre y escribe el reporte en `eval/results/`, con la línea de juicio humano por caso.
- **Juicio_Humano**: la marca mejor, igual o peor que el Dueño pone por caso en el reporte del Eval_de_Dominio, comparando con la respuesta de un modelo sin tools.
- **Linea_Base_Compras**: las métricas actuales de Dominio_Compras: 10/10 sin error, 9/10 en menos de 5 min, 1 pick falso de 47, unos 180 mil tokens por caso.
- **App**: la app de escritorio Tauri de `app/`.

## Requirements

### Requirement 1: Visión a 12 meses

**User Story:** Como Dueño, quiero una visión de 12 meses escrita y medible, para saber hacia dónde va Obsession y cuándo se cumplió el rumbo.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL declarar como visión a 12 meses que el Harness atiende Dominio_Compras y Dominio_Stack con el mismo Nucleo, y que cada Dominio se instala como Paquete_de_Dominio.
2. THE Hoja_de_Ruta SHALL definir la visión como cumplida cuando Dominio_Compras y Dominio_Stack superan cada uno su Puerta_de_Fase de eval con el mismo binario del Harness.
3. THE Hoja_de_Ruta SHALL declarar que la búsqueda de usuarios externos, el acompañante y el SaaS empiezan solo después de cumplida la visión y de la decisión de la Fase 4.
4. THE Hoja_de_Ruta SHALL listar las cinco Fases en orden (Fase 0 cierre mínimo de compras, Fase 1 separación del Nucleo, Fase 2 Dominio_Stack, Fase 3 validación de Dominio_Stack, Fase 4 decisión sobre acompañante y SaaS) con una duración estimada en meses para cada una, sumando como máximo 12 meses.
5. THE Hoja_de_Ruta SHALL describir en detalle (entregables, métricas de salida y dependencias del Dueño) la Fase 0, la Fase 1 y la Fase 2, y describir la Fase 3 y la Fase 4 con objetivo y métricas de salida.

### Requirement 2: Puertas entre fases

**User Story:** Como Dueño, quiero que cada fase tenga una puerta medible, para no empezar la siguiente sobre una base sin comprobar.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL definir para cada Fase una Puerta_de_Fase con métricas numéricas o comprobables por un comando del Harness.
2. WHEN una Fase cumple todas las métricas de su Puerta_de_Fase, THE Hoja_de_Ruta SHALL permitir el inicio de la Fase siguiente.
3. IF una Fase no cumple su Puerta_de_Fase al terminar su duración estimada, THEN THE Hoja_de_Ruta SHALL exigir que el Dueño registre en `docs/ESTADO.md` la métrica no cumplida, la causa conocida y la decisión tomada (extender la Fase, recortar su alcance o cambiar la meta).
4. THE Hoja_de_Ruta SHALL registrar el estado de cada Puerta_de_Fase en `docs/ESTADO.md` con la fecha y el comando o archivo que la comprueba.

### Requirement 3: Fase 0, cierre mínimo de Dominio_Compras

**User Story:** Como Dueño, quiero cerrar solo las dos piezas de compras que el resto del plan necesita, para pasar al segundo dominio sin dejar una base sin medir ni atada al repo.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL limitar la Fase 0 a dos piezas: el Juicio_Humano del eval de Dominio_Compras y el empaquetado de `packages/` como recursos de la App.
2. THE Hoja_de_Ruta SHALL justificar cada pieza elegida y cada candidato descartado (watch PR 1-2, Hiraoka, conector de AliExpress, panel de negociación) con el motivo de su inclusión o exclusión.
3. THE Hoja_de_Ruta SHALL fijar como métrica de salida del Juicio_Humano que los 10 casos del reporte más reciente de `eval/results/` tienen marca mejor, igual o peor, con al menos 7 de 10 marcados mejor o igual.
4. IF el Juicio_Humano da menos de 7 de 10 casos mejor o igual, THEN THE Hoja_de_Ruta SHALL exigir que el Dueño registre en `docs/ESTADO.md` los casos peor y su causa antes de iniciar la Fase 2.
5. THE Hoja_de_Ruta SHALL fijar como métrica de salida del empaquetado que la App compilada, copiada a una carpeta fuera del repo y sin `OBSESSION_HOME`, carga los paquetes de serie (`core`, `tiendas-pe`, `tiendas-intl`, `temu-anyweb`) y completa una búsqueda.
6. THE Hoja_de_Ruta SHALL fijar para la Fase 0 una duración estimada de 1 mes como máximo.
7. WHERE el Dueño tiene tiempo disponible durante la Fase 0, THE Hoja_de_Ruta SHALL permitir aprobar Hiraoka en AnyWeb (`review` y `package`) sin que esa tarea bloquee la Puerta_de_Fase.

### Requirement 4: Fase 1, separación del Nucleo

**User Story:** Como Dueño, quiero separar el núcleo del harness de lo específico de compras, para que un dominio nuevo se agregue como paquete sin tocar el núcleo.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL definir como entregable de la Fase 1 un inventario de las piezas del Harness (módulos de `src/`, prompts de `prompts/`, skills de `packages/core/skills/` y casos de `eval/`) clasificadas como Nucleo o como Dominio_Compras.
2. THE Hoja_de_Ruta SHALL definir como entregable de la Fase 1 que los elementos de Dominio_Compras (normalización de precios en soles, tiendas, señales de falsificación, negociación, criterios de parada de compras) se mueven a Paquetes_de_Dominio de compras.
3. THE Hoja_de_Ruta SHALL definir como entregable de la Fase 1 que un Paquete_de_Dominio declara en `obsession-package.json` los prompts o fragmentos de prompt específicos del Dominio, el formato del veredicto y los casos de su Eval_de_Dominio.
4. THE Hoja_de_Ruta SHALL fijar como métrica de salida de la Fase 1 que `cargo test` pasa y que `obsession eval` de Dominio_Compras repite la Linea_Base_Compras: 10/10 sin error, como máximo 1 pick falso por cada 40 picks y un promedio de tokens por caso dentro de ±10 % de 180 mil.
5. THE Hoja_de_Ruta SHALL fijar como métrica de salida de la Fase 1 que `obsession negotiate eval` mantiene 0 violaciones de límite, 0 compromisos sin checkpoint y 0 fugas de datos.
6. THE Hoja_de_Ruta SHALL fijar como métrica de salida de la Fase 1 que el Nucleo carga y corre un Paquete_de_Dominio de prueba con un experto y un caso de eval, sin referencias a Dominio_Compras.
7. IF la separación del Nucleo empeora la Linea_Base_Compras fuera de los márgenes del criterio 4, THEN THE Hoja_de_Ruta SHALL exigir que la causa se corrija o se registre en `docs/ESTADO.md` antes de iniciar la Fase 2.
8. THE Hoja_de_Ruta SHALL fijar para la Fase 1 una duración estimada de 2 meses como máximo.

### Requirement 5: Fase 2, Dominio_Stack

**User Story:** Como Dueño, quiero que Obsession elija stack de software con evidencia verificada, para demostrar que la obsesión acotada sirve fuera de compras.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL definir el alcance de Dominio_Stack como comparar entre 2 y 5 librerías o herramientas candidatas para una necesidad técnica escrita por el usuario, y entregar un veredicto con una recomendación, las alternativas descartadas y el motivo de cada descarte.
2. THE Hoja_de_Ruta SHALL definir las fuentes de Evidencia de Dominio_Stack: documentación oficial, registros de paquetes (por ejemplo crates.io y npm), repositorios e issues de GitHub y benchmarks publicados con metodología visible.
3. THE Hoja_de_Ruta SHALL exigir que cada afirmación del veredicto de Dominio_Stack cite una Evidencia con URL https que el Harness volvió a abrir durante la corrida.
4. THE Hoja_de_Ruta SHALL definir para Dominio_Stack los equivalentes de las piezas de Dominio_Compras: expertos por tipo de fuente, recomprobación de Evidencia, verificador de otra familia de modelo y ranker con rondas, todos provistos por el Nucleo.
5. THE Hoja_de_Ruta SHALL exigir que Dominio_Stack se entregue como Paquete_de_Dominio instalable con `obsession install` sin cambios en el Nucleo.
6. IF construir Dominio_Stack requiere un cambio en el Nucleo, THEN THE Hoja_de_Ruta SHALL exigir que el cambio sea general para cualquier Dominio y que `obsession eval` de Dominio_Compras se vuelva a correr con los márgenes del Requirement 4.
7. THE Hoja_de_Ruta SHALL definir que un benchmark sin metodología visible o publicado por el autor de una de las candidatas se marca en el veredicto como Evidencia de parte.
8. THE Hoja_de_Ruta SHALL fijar para la Fase 2 una duración estimada de 3 meses como máximo.

### Requirement 6: Fase 3, validación de Dominio_Stack

**User Story:** Como Dueño, quiero un eval fijo para elegir stack, para comparar Obsession contra un modelo sin tools con el mismo método que en compras.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL definir un Eval_de_Dominio para Dominio_Stack con 10 casos fijos en un archivo de `eval/`, cada uno con la necesidad técnica, las candidatas esperadas y las restricciones (lenguaje, licencia, plataforma).
2. THE Hoja_de_Ruta SHALL exigir que el reporte del Eval_de_Dominio de Dominio_Stack se escriba en `eval/results/` con el mismo formato del eval de compras: tabla lado a lado con la respuesta de un modelo sin tools y la línea de Juicio_Humano por caso.
3. THE Hoja_de_Ruta SHALL fijar como métricas de salida de la Fase 3: 10/10 casos sin error, como máximo 1 Afirmacion_Falsa por cada 40 afirmaciones citadas, un promedio de como máximo 250 mil tokens por caso y al menos 8 de 10 casos en menos de 10 min.
4. THE Hoja_de_Ruta SHALL fijar como métrica de salida de la Fase 3 que el Juicio_Humano marca al menos 7 de 10 casos de Dominio_Stack como mejor o igual que el modelo sin tools.
5. WHEN el Eval_de_Dominio de Dominio_Stack cumple las métricas de los criterios 3 y 4, THE Hoja_de_Ruta SHALL dar por cumplida la visión del Requirement 1.
6. THE Hoja_de_Ruta SHALL prohibir cambiar los casos fijos del Eval_de_Dominio entre corridas comparadas, igual que en `eval/cases.json`.
7. THE Hoja_de_Ruta SHALL fijar para la Fase 3 una duración estimada de 2 meses como máximo.

### Requirement 7: Fase 4, decisión sobre acompañante y SaaS

**User Story:** Como Dueño, quiero decidir con datos si sigo con acompañante y SaaS, para no construirlos sin usuarios ni sin un harness probado.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL definir la Fase 4 como una decisión escrita, sin construcción de acompañante ni de SaaS dentro del horizonte de 12 meses.
2. THE Hoja_de_Ruta SHALL exigir que la decisión de la Fase 4 use como entrada los reportes de eval de los dos Dominios, el costo en tokens por caso y el tiempo del Dueño gastado en aprobaciones y juicios.
3. THE Hoja_de_Ruta SHALL exigir que la decisión de la Fase 4 nombre a qué usuarios externos se buscaría primero y qué métrica de uso real se mediría con ellos.
4. IF la Fase 3 no cumple su Puerta_de_Fase, THEN THE Hoja_de_Ruta SHALL exigir que la Fase 4 decida entre otra iteración de Dominio_Stack y volver a concentrarse en Dominio_Compras, antes de considerar acompañante o SaaS.

### Requirement 8: Limitaciones y riesgos

**User Story:** Como Dueño, quiero las limitaciones y riesgos escritos con su mitigación, para planificar sin sorpresas.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL listar las limitaciones técnicas actuales del Harness: funciona solo en Windows para claves DPAPI, programador de tareas y toasts; depende de magpie como proveedor por defecto; BYOK no se probó con un proveedor real; Amazon, AliExpress y Temu bloquean el acceso sin navegador; AliExpress cambia precios entre búsquedas.
2. THE Hoja_de_Ruta SHALL listar las limitaciones de capacidad: un solo desarrollador, el Juicio_Humano depende de un solo juez y las aprobaciones de AnyWeb son humanas por diseño.
3. THE Hoja_de_Ruta SHALL listar los riesgos propios de Dominio_Stack: documentación que cambia entre versiones, límites de la API de GitHub (60 peticiones por hora sin token, 5000 con token), benchmarks de parte o desactualizados y casos de eval cuya respuesta correcta cambia con el tiempo.
4. THE Hoja_de_Ruta SHALL asociar a cada riesgo listado una mitigación concreta y la Fase donde se aplica.
5. THE Hoja_de_Ruta SHALL declarar como riesgo que la separación del Nucleo degrade Dominio_Compras, con la mitigación de correr `obsession eval` y `obsession negotiate eval` como parte de la Puerta_de_Fase de la Fase 1.
6. THE Hoja_de_Ruta SHALL declarar como riesgo que un Juicio_Humano de un solo juez sesgue la métrica de "mejor", y registrar esa limitación junto a cada resultado de Juicio_Humano.
7. THE Hoja_de_Ruta SHALL mantener la exclusión de herramientas de evasión anti-bot en todos los Dominios.

### Requirement 9: Dependencias del Dueño

**User Story:** Como Dueño, quiero ver qué depende de mí en cada fase, para reservar ese tiempo y no bloquear el plan.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL listar por Fase las tareas que solo el Dueño puede hacer, con una estimación de horas por tarea.
2. THE Hoja_de_Ruta SHALL incluir entre las tareas del Dueño: el Juicio_Humano de Dominio_Compras (Fase 0), el Juicio_Humano de Dominio_Stack (Fase 3), la redacción de los 10 casos fijos de Dominio_Stack o su aprobación (Fase 3) y la decisión escrita de la Fase 4.
3. THE Hoja_de_Ruta SHALL listar como tareas del Dueño sin Fase asignada las que no bloquean ninguna Puerta_de_Fase: reglas de negociación R1-R10, clave de afiliado de AliExpress y aprobación de Hiraoka en AnyWeb.
4. IF una tarea del Dueño bloquea una Puerta_de_Fase durante más de 2 semanas, THEN THE Hoja_de_Ruta SHALL exigir que el bloqueo quede registrado en `docs/ESTADO.md` con la tarea y la fecha.

### Requirement 10: No-objetivos

**User Story:** Como Dueño, quiero los no-objetivos explícitos, para que el plan no crezca durante los 12 meses.

#### Acceptance Criteria

1. THE Hoja_de_Ruta SHALL declarar como no-objetivos del horizonte de 12 meses: el acompañante, el SaaS, la búsqueda activa de usuarios externos, el poller remoto del watch (PR 4 de `docs/design/watch.md`) y la negociación autónoma en plataformas reales.
2. THE Hoja_de_Ruta SHALL declarar como pendientes de Dominio_Compras fuera de las Fases 0 a 3: watch PR 1-3, panel de negociación (PR 4 de `docs/design/negotiation.md`), mejora del tono de negociación, conector de AliExpress por API y fuentes nuevas de compras.
3. THE Hoja_de_Ruta SHALL declarar como no-objetivo un tercer Dominio dentro del horizonte de 12 meses.
4. WHEN el Dueño propone agregar trabajo que figura en los no-objetivos, THE Hoja_de_Ruta SHALL exigir que el cambio quede escrito en la Hoja_de_Ruta con la Fase afectada y el trabajo que sale para hacerle espacio.
