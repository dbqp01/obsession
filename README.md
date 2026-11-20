# Obsession (MVP)

Busca la mejor compra, no solo el precio más bajo.

```
cargo run --release -- "audífonos over-ear para música en casa, hasta 350 soles, Cusco"
obsession experts   # catálogo de expertos
obsession skills    # skills del harness
obsession eval      # corre los 10 casos fijos (o: obsession eval iem-80)
```

## App de escritorio (Tauri)

```
cd app
cargo run
```

La app usa el mismo núcleo (`src/lib.rs`) que la CLI y muestra la búsqueda paso a paso. Primero aparece lo que entendió (necesidad, precio justo, candidatos). Después se ve qué busca cada experto, qué páginas se volvieron a abrir y cuáles se descartaron. Al final llega el veredicto, con la confianza, los avisos explicados y el enlace a la tienda. Lee `experts/`, `skills/` y `mcp.json` de este repo, o de `OBSESSION_HOME` si está definido.

Para revisar la interfaz en un navegador sin la app, graba una corrida con `OBSESSION_RECORD=ui/demo-events.jsonl obsession "…"` y abre `ui/index.html` desde un servidor estático: al pulsar Buscar, la reproduce.

## Conexiones (tiendas con tu sesión)

Algunas tiendas muestran precios solo con tu cuenta. Un MCP en `mcp.json` que tiene un bloque `login` aparece en el panel **Conexiones** de la app y en `obsession connections`. Al conectar se abre el login del propio conector (hoy el de AnyWeb para Temu): una ventana normal del navegador con la página oficial de la tienda, donde inicias sesión tú. Obsession no ve contraseñas ni cookies; solo comprueba que el perfil del navegador exista. "Desconectar" (`obsession logout temu`) borra ese perfil.

Los expertos de una tienda sin sesión no participan en la búsqueda, y la app avisa por qué.

Límite: Chrome permite un solo proceso por perfil. Si otro programa ya tiene abierto el MCP de Temu (por ejemplo, un cliente MCP con `anyweb-temu` registrado), el experto de Temu falla con `INTERNAL_ERROR` hasta que ese programa lo cierre.

## Tus propias claves (BYOK)

Sin magpie, puedes usar cualquier proveedor compatible con OpenAI (OpenAI, DeepSeek, Moonshot, Zhipu, OpenRouter, o Anthropic por un endpoint compatible):

```
obsession provider add openrouter https://openrouter.ai/api/v1
obsession key set openrouter        # lee la clave de stdin, nunca de los argumentos
obsession provider list             # nombre, URL y si hay clave (nunca la clave)
obsession key remove openrouter
```

Un modelo escrito `byok:<proveedor>/<modelo>` (por ejemplo `byok:openrouter/deepseek/deepseek-chat`) va a ese proveedor; cualquier otro id sigue yendo a magpie como hoy. Úsalo en el `model:` de un experto o en `OBSESSION_MAIN` / `OBSESSION_VERIFIER`.

Los proveedores se guardan sin secretos en `%LOCALAPPDATA%\Obsession\providers.json`. Cada clave va cifrada con DPAPI (solo tu usuario de Windows la descifra) en `%LOCALAPPDATA%\Obsession\keys\<proveedor>.key`. La URL debe ser https; http solo para localhost. Fuera de Windows no se guardan claves: usa `OBSESSION_KEY_<PROVEEDOR>`.

## Eval

`obsession eval` corre las consultas de `eval/cases.json`. A cada pick lo vuelve a abrir en su página y lo marca como falso si no abre, si el precio se movió más de 5 % o si es un accesorio. También pide la respuesta a un modelo sin tools, para comparar. El resultado queda en `eval/results/<fecha>.md`, una tabla lado a lado donde tú marcas, caso por caso, si Obsession eligió mejor, igual o peor.

## Tres capas

| Capa | Qué es | Dónde |
|---|---|---|
| **MCP** (las manos) | Servidores aparte, sin LLM. Cada tienda expone `search` por stdio (`obsession mcp amazon`). | `mcp.json` |
| **Expertos** (agentes) | Un `.md` con frontmatter (`name`, `description`, `model`, `mcp`, `skills`, `max_steps`) y su prompt. Solo el orquestador los invoca. | `experts/*.md` |
| **Skills** (cómo trabajar) | `SKILL.md` en formato estándar (Agent Skills) que el experto carga cuando la necesita. | `skills/*/SKILL.md` |

Cada experto decide sus propias búsquedas, solo ve sus propios MCP y debe terminar con `submit`.

## Flujo

1. **Planificador:** arma el dossier (necesidad, precio justo, candidatos, saltos de categoría) y elige expertos. Solo ve el nombre y la descripción de cada experto.
2. **Expertos en paralelo:** cada uno corre su loop de agente sobre sus MCP y entrega `picks`, `tried` y `missing`.
3. **Páginas:** cada pick se vuelve a abrir con la tool `open` de su tienda. Si la página no lo confirma o no hay stock, se descarta. El precio y el vendedor de la página reemplazan a los de la búsqueda. AliExpress necesita JavaScript, así que ahí solo se confirma que el producto reaparece al buscarlo de nuevo.
4. **Verificador:** revisa solo los picks, con un modelo de otra familia para no compartir sesgos. Si falla, reintenta con el modelo principal.
5. **Ranker:** decide. Si no está satisfecho, pide otra ronda con una pista y otros expertos (máximo 2 rondas).
6. Todo queda en `obsession.db` y en `runs/run-N.csv`.

## Contexto y decisión

- Los resultados de búsqueda no entran crudos al modelo. Se guardan en un store y el experto recibe una tabla de una línea por listing. Si quiere el detalle, llama a `inspect(id)`.
- Las skills se cargan bajo demanda: el prompt solo trae su nombre y su descripción.
- Contra la indecisión, cada experto tiene un número máximo de pasos. Si responde en prosa, el harness le exige una tool. En el último paso solo puede llamar a `submit`.
- `submit` descarta los ids que el experto nunca vio, así que no puede inventar listings.

## Notas

- Tiendas: Amazon, AliExpress, Falabella y Plaza Vea. Mercado Libre y Temu bloquean con anti-bot, y no se evade.
- Los modelos van por el router magpie (`MAGPIE_URL`, por defecto `http://127.0.0.1:3425/v1`). Cada experto define su modelo en el frontmatter. `OBSESSION_MAIN` y `OBSESSION_VERIFIER` cambian el planificador y el verificador.
- Cada corrida tiene un tope de tokens (`OBSESSION_MAX_TOKENS`, por defecto 400000). Al agotarse, los expertos entregan `submit` con lo que tienen y no se hace la ronda 2. El reporte final y `eval` muestran los tokens usados.
- Para agregar un MCP externo: añade una entrada en `mcp.json` y nómbrala en el `mcp:` de un experto. Si devuelve listings con la forma `store, title, price_pen, url, seller, rating, sold`, entra al store. Si devuelve otra cosa, el texto se trunca.
- `prompts/` y `prompts/expert_harness.md` se compilan dentro del binario: después de editarlos hay que recompilar. `experts/` y `skills/` se leen al ejecutar.
