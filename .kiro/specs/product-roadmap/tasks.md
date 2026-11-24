# Implementation Plan: Hoja de ruta de Obsession (12 meses)

## Overview

El plan convierte el diseño en pasos de código incrementales, en Rust (edición 2024) para el Harness y Node 24 para el MCP de Dominio_Stack. Las Fases 0 y 1 van en detalle: documento de la Hoja_de_Ruta, rutas portables de la App, lectura de marcas del juez, separación del Nucleo, bloque `domain` del manifiesto, ítem y recomprobación genéricos, eval de dominio y prueba de Nucleo neutral. Las Fases 2 y 3 quedan en tareas más gruesas, que se detallarán cuando pase la Puerta_de_Fase de la Fase 1. La Fase 4 no tiene tareas de código.

Las tareas que solo puede hacer el Dueño (juicios, aprobaciones, pruebas manuales, decisiones) no son tareas de código y están en la sección "Tareas del Dueño" de las notas, marcadas con **[Dueño]**. Cada checkpoint indica qué tarea del Dueño necesita para cerrar su Puerta_de_Fase.

Los property tests usan `proptest` con al menos 100 casos y llevan el comentario `Feature: product-roadmap, Property N: <título>`. Cada property test va en su propio archivo de `tests/` para que las tareas no choquen en el mismo archivo.

## Tasks

### Fase 0: cierre mínimo de Dominio_Compras

- [ ] 1. Documentos de la Hoja_de_Ruta
  - [ ] 1.1 Escribir `docs/ROADMAP.md`
    - Secciones en el orden del diseño (Componente 1): supuestos A1-A6 marcados "por confirmar", visión y criterio de cumplimiento, tabla de fases con duraciones (1 + 2 + 3 + 2 + 1 y 3 meses de holgura), una sección por fase (detalle completo para Fases 0-2, objetivo y métricas para Fases 3-4), tabla de puertas, horas del Dueño, limitaciones y riesgos con mitigación y fase, no-objetivos, pendientes de compras y tabla "Cambios"
    - Incluir la justificación de las dos piezas de la Fase 0 y de cada candidato descartado (watch PR 1-2, Hiraoka, conector de AliExpress, panel de negociación)
    - Incluir el procedimiento de la prueba manual de la App portable
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 2.1, 3.1, 3.2, 3.6, 3.7, 4.8, 5.8, 6.7, 7.1, 7.2, 7.3, 7.4, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 9.1, 9.2, 9.3, 10.1, 10.2, 10.3, 10.4_

  - [ ] 1.2 Agregar a `docs/ESTADO.md` la sección "Puertas de fase"
    - Tabla de puertas (Fase, Métrica, Meta, Resultado, Fecha, Comprobación, Estado), tabla "Desvíos" y tabla "Bloqueos del Dueño (más de 2 semanas)", según el Componente 6 del diseño
    - Filas iniciales de la Fase 0 en estado `en curso`; no tocar el resto del archivo
    - _Requirements: 2.2, 2.3, 2.4, 3.4, 4.7, 9.4_

  - [ ]* 1.3 Escribir test de la tabla de fases
    - `tests/roadmap.rs`: lee `docs/ROADMAP.md`, extrae las duraciones de la tabla de fases y comprueba que hay cinco fases en orden y que suman como máximo 12 meses
    - _Requirements: 1.4_

- [ ] 2. Rutas portables de la App
  - [ ] 2.1 Implementar `Paths` y `resolve_paths` en `src/lib.rs`
    - Orden: `OBSESSION_HOME` (las dos rutas iguales) > recursos de la App > repo solo si se pasa (debug); `data` en `%LOCALAPPDATA%\Obsession` cuando no hay `OBSESSION_HOME`
    - Agregar `proptest` como `dev-dependency` en `Cargo.toml`
    - _Requirements: 3.5_

  - [ ]* 2.2 Write property test de resolución de rutas
    - `tests/props_paths.rs`
    - **Property 13: Resolución de rutas de la App**
    - **Validates: Requirements 3.5**

  - [ ] 2.3 Pasar `&Paths` en vez de `root` a `run`, `eval::run` y `packages::load`
    - Leer paquetes de serie desde `builtin`; escribir `obsession.db`, `runs/` y `eval/results/` en `data`
    - Actualizar el CLI de `src/main.rs` para construir `Paths` con `OBSESSION_HOME` o el directorio actual
    - _Requirements: 3.5_

  - [ ] 2.4 Empaquetar `packages/` como recursos de la App
    - `app/tauri.conf.json`: `"bundle": {"resources": {"../packages": "packages"}}`
    - `app/src/main.rs`: reemplazar `root()` por `resolve_paths` con `app.path().resource_dir()`; pasar el repo (`CARGO_MANIFEST_DIR/..`) solo con `cfg!(debug_assertions)`
    - En release sin recursos ni `OBSESSION_HOME`, mostrar el error "no encuentro los paquetes de serie" sin caer al repo
    - _Requirements: 3.5_

  - [ ]* 2.5 Write unit tests de los casos límite de rutas
    - `tests/paths.rs`: release sin recursos ni `OBSESSION_HOME` devuelve `None`; `data` fuera de `builtin`
    - _Requirements: 3.5_

- [ ] 3. Lectura de marcas del juez
  - [ ] 3.1 Implementar `judge_marks` y el subcomando `obsession eval judge <reporte.md>`
    - En `src/eval.rs`: leer la línea `**Juez (mejor / igual / peor):**` de cada caso, contar mejor, igual y peor, imprimir si se llega a 7 de 10 y la leyenda "Juicio de un solo juez"
    - Registrar el subcomando en `src/main.rs`
    - _Requirements: 3.3, 3.4, 8.6_

  - [ ]* 3.2 Write property test de las marcas del juez
    - `tests/props_judge.rs`
    - **Property 11: Ida y vuelta de las marcas del juez**
    - **Validates: Requirements 3.3, 6.2, 6.4**

- [ ] 4. Checkpoint Fase 0 - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
  - La Puerta_de_Fase 0 además necesita las tareas del Dueño D0.1 y D0.2 (ver notas).

### Fase 1: separación del Nucleo

- [ ] 5. Estructura de módulos `core` y `compras`
  - [ ] 5.1 Crear `src/core/` y `src/compras/` y mover módulos sin cambiar comportamiento
    - A `src/core/`: `llm.rs`, `mcp.rs`, `providers.rs`, `connections.rs`, `packages.rs`, `agent.rs`, `db.rs`, `eval.rs`
    - A `src/compras/`: `stores.rs`, `negotiate.rs`, `negsim.rs`; mover `prompts/negotiator.md`, `seller_sim.md` y `tone_judge.md` a `src/compras/prompts/` y ajustar los `include_str!`
    - `cargo test` debe pasar igual que antes del movimiento
    - _Requirements: 4.1, 4.2_

- [ ] 6. Bloque `domain` del manifiesto
  - [ ] 6.1 Agregar `DomainSpec`, `DomainPrompts`, `RecheckRule`, `EvalSpec` y `GateMetric` a `src/core/packages.rs`
    - Campo `domain` opcional con `deny_unknown_fields`; validar cada ruta con `inside`; comprobar que los prompts y el esquema existen
    - Choque de `domain.id` entre paquetes con el mismo aviso de `clash`; error de `--domain` desconocido con la lista de Dominios cargados
    - Leer `domain:` del frontmatter de cada experto, con `compras` por omisión
    - _Requirements: 4.3, 5.5_

  - [ ]* 6.2 Write property test de compatibilidad de manifiestos
    - `tests/props_manifest.rs`
    - **Property 1: Los manifiestos actuales siguen cargando igual**
    - **Validates: Requirements 4.3, 4.4**

  - [ ]* 6.3 Write property test de rutas del Dominio
    - `tests/props_domain_paths.rs`
    - **Property 2: Las rutas del Dominio no salen del paquete**
    - **Validates: Requirements 4.3, 5.5**

- [ ] 7. Ítem y recomprobación genéricos
  - [ ] 7.1 Implementar `Item` y `Check` en `src/core/agent.rs`
    - `attrs` con `#[serde(flatten)]` y `source` con `#[serde(alias = "store")]`, para que el JSON de `Listing` y `PageCheck` de las tiendas siga siendo válido
    - `Store::table` muestra las columnas de `domain.table_columns`; `anyweb_items` y `text_listings` producen `Item`
    - Reescribir `prompts/expert_harness.md` sin "tienda" ni columna de precio fija
    - _Requirements: 4.2, 4.4_

  - [ ]* 7.2 Write property test de ida y vuelta Listing ↔ Item
    - `tests/props_item.rs`
    - **Property 3: Ida y vuelta Listing ↔ Item**
    - **Validates: Requirements 4.2, 4.4**

  - [ ] 7.3 Implementar `recheck_problem` en `src/core/recheck.rs`
    - Reglas `Relative`, `Exact` y `Required`; un `Check` con `BLOCKED` en la nota devuelve `None` y se cuenta aparte
    - Reemplazar `price_matches` por la regla `Relative { attr: "price_pen", tolerance: 0.05 }`
    - _Requirements: 4.4, 8.7_

  - [ ]* 7.4 Write property test de la regla relativa
    - `tests/props_recheck.rs`
    - **Property 4: La regla relativa reproduce `price_matches`**
    - **Validates: Requirements 4.4, 6.3**

  - [ ]* 7.5 Write property test de bloqueos
    - `tests/props_blocked.rs`
    - **Property 5: Un bloqueo nunca cuenta como falso**
    - **Validates: Requirements 4.4, 6.3, 8.7**

- [ ] 8. Paquete `compras`
  - [ ] 8.1 Crear `packages/compras/` con su manifiesto y archivos de Dominio
    - `obsession-package.json` con el bloque `domain` del diseño (prompts, `table_columns`, `recheck`, `verdict_schema`, `eval` con `time_limit_s: 300` y la puerta de la Fase 1)
    - Mover `prompts/planner.md`, `verifier.md` y `ranker.md` a `packages/compras/prompts/`; sacar la constante `BASELINE` de `eval.rs` a `prompts/baseline.md`; escribir `verdict.schema.json` con la forma actual del ranker
    - Mover `eval/cases.json` a `packages/compras/eval/cases.json` sin cambiar el contenido
    - Mover las skills `price-normalization`, `counterfeit-signals` y `negotiation` de `packages/core/skills/` a `packages/compras/skills/`
    - _Requirements: 4.2, 4.3_

  - [ ] 8.2 Separar las skills mixtas y actualizar los expertos
    - `report-findings` en `core` sin ejemplos de compras; ejemplos en `packages/compras/skills/report-findings-compras/`
    - `stop-criteria` en `core` con regla 1 genérica; regla de compras en `packages/compras/skills/stop-criteria-compras/`
    - Cambiar el frontmatter `skills:` de los expertos de `tiendas-pe`, `tiendas-intl`, `temu-anyweb` y `comparadores-pe`
    - _Requirements: 4.1, 4.2_

  - [ ]* 8.3 Write unit tests de carga de paquetes
    - Actualizar `repo_packages_load_every_builtin_expert` con el paquete `compras`
    - Comprobar que `core` no tiene skills de compras y que `compras` sí
    - _Requirements: 4.2_

- [ ] 9. Corrida y eval guiados por el Dominio
  - [ ] 9.1 Mover `run` a `src/core/run.rs` y leer prompts y reglas del `DomainSpec`
    - `run`, `pick`, `verify`, `open_pages` y `verified_listings` usan `Item`, `recheck_problem` y los prompts del Dominio en vez de `include_str!`
    - Agregar `--domain <id>` a `obsession run` en `src/main.rs`, con `compras` por omisión
    - Si el veredicto no cumple `verdict_schema`: error en ronda 1, se mantiene la decisión anterior en ronda 2
    - _Requirements: 4.2, 4.3, 4.4_

  - [ ] 9.2 Migrar `obsession.db` en `src/core/db.rs`
    - Copiar el archivo a `obsession.db.bak` antes de migrar; agregar la columna `attrs TEXT` a `listings`; leer filas viejas (`attrs` en `NULL`) con sus columnas actuales
    - _Requirements: 4.4_

  - [ ] 9.3 Hacer genérico el eval en `src/core/eval.rs`
    - `obsession eval --domain <id>` lee `domain.eval.cases`, recomprueba con `open` y `recheck_problem` y pide la respuesta sin tools con `prompts.baseline`
    - Resumen con `cases`, `completed`, `under_limit`, picks o claims, falsos, bloqueados y `tokens_avg`; `false_per_40` no cumple con total 0
    - Implementar `gate` y escribir la tabla "Puerta" al final del `.md`; escribir `cases_sha256` y el modelo de cada rol en el `.json`
    - Nombre del reporte de compras: `<stamp>.md` hasta cerrar la Fase 1, luego `compras-<stamp>.md`
    - _Requirements: 4.3, 4.4, 4.7, 6.2_

  - [ ]* 9.4 Write property test de la puerta
    - `tests/props_gate.rs`
    - **Property 9: La puerta pasa si y solo si todas las métricas cumplen**
    - **Validates: Requirements 1.2, 2.2, 2.3, 3.4, 4.4, 4.7, 6.3, 6.5**

  - [ ]* 9.5 Write property test del resumen de eval
    - `tests/props_summary.rs`
    - **Property 10: Invariantes del resumen de eval**
    - **Validates: Requirements 4.4, 6.3**

  - [ ] 9.6 Implementar `obsession eval compare <a.json> <b.json>`
    - Se niega a comparar si `cases_sha256` difiere y muestra las dos huellas
    - _Requirements: 6.6_

  - [ ]* 9.7 Write property test de la huella de casos
    - `tests/props_cases_hash.rs`
    - **Property 12: No se comparan corridas con casos distintos**
    - **Validates: Requirements 6.6**

  - [ ] 9.8 Implementar `cite_claims` y `classify_benchmark` en `src/core/claims.rs`
    - Se agregan en la Fase 1 para que la Fase 2 no toque el Nucleo
    - Claims sin ids, con ids inexistentes, con URL no https o con recomprobación fallida pasan a "Afirmaciones sin evidencia" y cuentan como falsos en el eval
    - _Requirements: 5.3, 5.6, 5.7_

  - [ ]* 9.9 Write property test de afirmaciones citadas
    - `tests/props_claims.rs`
    - **Property 6: Solo llegan al veredicto afirmaciones con evidencia recomprobada**
    - **Validates: Requirements 5.3**

  - [ ]* 9.10 Write property test de benchmarks de parte
    - `tests/props_benchmark.rs`
    - **Property 7: Benchmarks de parte**
    - **Validates: Requirements 5.7**

- [ ] 10. Prueba de Nucleo neutral
  - [ ] 10.1 Agregar dobles de prueba para MCP y LLM
    - `obsession mcp fake` responde `search` y `open` con datos fijos; un `llm` falso se activa con una variable de entorno de test en `src/core/llm.rs`
    - _Requirements: 4.6_

  - [ ] 10.2 Escribir los tests de la puerta del Nucleo
    - `core_runs_a_neutral_package`: paquete `prueba` en carpeta temporal con `domain.id = "prueba"`, un experto, el MCP falso y un caso de eval; comprueba un pick recomprobado y un reporte con línea de juez
    - `core_has_no_shopping_imports`: recorre `src/core/**/*.rs` y falla si encuentra `crate::compras`, `price_pen`, `stores::` o `soles`
    - No son opcionales: son métricas de la Puerta_de_Fase 1
    - _Requirements: 4.6_

- [ ] 11. Checkpoint Fase 1 - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
  - La Puerta_de_Fase 1 además necesita `obsession eval --domain compras` y `obsession negotiate eval` con modelos reales y su revisión por el Dueño (D1.1 y D1.2 en notas).
  - _Requirements: 4.4, 4.5, 4.7_

### Fase 2: Dominio_Stack (tareas gruesas, se detallan al pasar la Puerta_de_Fase 1)

- [ ] 12. Paquete `packages/stack`
  - [ ] 12.1 Crear manifiesto, prompts, esquema, expertos y skills de Stack
    - `obsession-package.json` con `domain.id = "stack"`, reglas de recomprobación A6, `time_limit_s: 600` y la puerta de la Fase 3
    - Prompts `planner`, `verifier`, `ranker`, `baseline`; `verdict.schema.json`; expertos `registries`, `github`, `docs`, `benchmarks`; skills `evidence-citation`, `partisan-benchmarks`, `stop-criteria-stack`
    - _Requirements: 5.1, 5.2, 5.4, 5.5, 5.7_

  - [ ] 12.2 Implementar `packages/stack/mcp/server.mjs` (Node 24)
    - Un solo servidor con argumento `registry`, `github`, `docs` o `bench`; tools `search`, `repo`, `issues`, `fetch` y `open` según la tabla del diseño
    - `GITHUB_TOKEN` opcional; caché por URL de 10 minutos; 403 con `X-RateLimit-Remaining: 0` reportado como `BLOCKED`; sin reintentos con otra identidad ni evasión anti-bot
    - _Requirements: 5.2, 5.3, 8.3, 8.7_

  - [ ] 12.3 Validar la forma del veredicto de Stack
    - 2 a 5 candidatas distintas, recomendación entre ellas, cada otra candidata una vez en `discarded` con motivo no vacío
    - Si la validación necesita código del Nucleo, debe ser general y obliga a volver a correr `obsession eval --domain compras`
    - _Requirements: 5.1, 5.6_

  - [ ]* 12.4 Write property test de la forma del veredicto
    - `tests/props_verdict.rs`
    - **Property 8: Forma del veredicto de Stack**
    - **Validates: Requirements 5.1**

  - [ ]* 12.5 Write integration test de instalación
    - `obsession install packages/stack` en una carpeta temporal de usuario, sin recompilar; actualizar `repo_packages_load_every_builtin_expert` con `stack`
    - _Requirements: 5.5_

- [ ] 13. Checkpoint Fase 2 - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
  - La Puerta_de_Fase 2 además necesita la corrida piloto de 3 necesidades con `obsession run --domain stack` y su revisión por el Dueño (D2.1).

### Fase 3: validación de Dominio_Stack (tareas gruesas)

- [ ] 14. Eval de Dominio_Stack
  - [ ] 14.1 Crear `packages/stack/eval/cases.json`
    - Estructura del diseño (`id`, `need`, `expected_candidates`, `constraints {language, license, platform}`); el contenido de los 10 casos lo redacta o aprueba el Dueño (D3.1)
    - _Requirements: 6.1, 6.6_

  - [ ]* 14.2 Write unit test de los casos de Stack
    - 10 casos con `need`, 2 a 5 `expected_candidates` y `constraints` completas
    - _Requirements: 6.1_

- [ ] 15. Checkpoint final - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.
  - La Puerta_de_Fase 3 además necesita `obsession eval --domain stack` con modelos reales y el Juicio_Humano (D3.2).

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP. La excepción deliberada es 10.2: son tests, pero no son opcionales porque forman parte de la Puerta_de_Fase 1.
- Cada tarea referencia criterios de `requirements.md`; las propiedades vienen de la sección "Correctness Properties" de `design.md`.
- La Fase 4 es una decisión escrita (`docs/decisions/fase-4.md`) y no tiene tareas de código.
- Los supuestos A1-A6 del diseño siguen sin confirmar. Si el Dueño cambia alguno, se ajustan los valores en `docs/ROADMAP.md` (1.1) y en las puertas de los manifiestos (8.1, 12.1).

### Tareas del Dueño (no son tareas de código)

- **[Dueño] D0.1** Juicio_Humano de los 10 casos del último reporte de `eval/results/` de compras (3-4 h). Comprobar con `obsession eval judge` (3.1). Si da menos de 7/10, registrar los casos peor y su causa en `docs/ESTADO.md` antes de la Fase 2.
- **[Dueño] D0.2** Prueba manual de la App compilada, copiada fuera del repo y sin `OBSESSION_HOME`: carga `core`, `tiendas-pe`, `tiendas-intl`, `temu-anyweb` y completa una búsqueda (1 h).
- **[Dueño] D1.1** Revisar el inventario Nucleo/Compras antes de la tarea 5.1 (2 h).
- **[Dueño] D1.2** Correr y revisar `obsession eval --domain compras` y `obsession negotiate eval` (1 h, más unos 30 min de corrida cada uno).
- **[Dueño] D2.1** Revisar los veredictos de la corrida piloto de Stack (2-3 h).
- **[Dueño] D3.1** Redactar o aprobar los 10 casos fijos de Stack (3-4 h).
- **[Dueño] D3.2** Juicio_Humano de los 10 casos de Stack (unos 5 h).
- **[Dueño] D4.1** Decisión escrita de la Fase 4 (3-4 h).
- **[Dueño] sin fase, no bloquean puertas:** reglas de negociación R1-R10, clave de afiliado de AliExpress, `review` y `package` de Hiraoka en AnyWeb.
- **[Dueño] siempre:** copiar las filas de la tabla "Puerta" de cada reporte a `docs/ESTADO.md` y registrar desvíos y bloqueos de más de 2 semanas.

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2", "2.1"] },
    { "id": 1, "tasks": ["1.3", "2.2", "2.3"] },
    { "id": 2, "tasks": ["2.4", "2.5", "3.1"] },
    { "id": 3, "tasks": ["3.2"] },
    { "id": 4, "tasks": ["4"] },
    { "id": 5, "tasks": ["5.1"] },
    { "id": 6, "tasks": ["6.1"] },
    { "id": 7, "tasks": ["6.2", "6.3", "7.1"] },
    { "id": 8, "tasks": ["7.2", "7.3", "8.1"] },
    { "id": 9, "tasks": ["7.4", "7.5", "8.2", "9.2"] },
    { "id": 10, "tasks": ["8.3", "9.1"] },
    { "id": 11, "tasks": ["9.3", "9.8"] },
    { "id": 12, "tasks": ["9.4", "9.5", "9.6", "9.9", "9.10"] },
    { "id": 13, "tasks": ["9.7", "10.1"] },
    { "id": 14, "tasks": ["10.2"] },
    { "id": 15, "tasks": ["11"] },
    { "id": 16, "tasks": ["12.1", "12.2"] },
    { "id": 17, "tasks": ["12.3"] },
    { "id": 18, "tasks": ["12.4", "12.5"] },
    { "id": 19, "tasks": ["13"] },
    { "id": 20, "tasks": ["14.1"] },
    { "id": 21, "tasks": ["14.2"] },
    { "id": 22, "tasks": ["15"] }
  ]
}
```
