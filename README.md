# Obsession (MVP)

Busca la mejor compra, no solo el precio más bajo.

```
cargo run --release -- "audífonos over-ear para música en casa, hasta 350 soles, Cusco"
obsession experts   # catálogo de expertos
obsession skills    # skills del harness
```

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
3. **Verificador:** revisa solo los picks, con un modelo de otra familia para no compartir sesgos. Si falla, reintenta con el modelo principal.
4. **Ranker:** decide. Si no está satisfecho, pide otra ronda con una pista y otros expertos (máximo 2 rondas).
5. Todo queda en `obsession.db` y en `run-N.csv`.

## Contexto y decisión

- Los resultados de búsqueda no entran crudos al modelo. Se guardan en un store y el experto recibe una tabla de una línea por listing. Si quiere el detalle, llama a `inspect(id)`.
- Las skills se cargan bajo demanda: el prompt solo trae su nombre y su descripción.
- Contra la indecisión, cada experto tiene un número máximo de pasos. Si responde en prosa, el harness le exige una tool. En el último paso solo puede llamar a `submit`.
- `submit` descarta los ids que el experto nunca vio, así que no puede inventar listings.

## Notas

- Tiendas: Amazon, AliExpress, Falabella y Plaza Vea. Mercado Libre y Temu bloquean con anti-bot, y no se evade.
- Los modelos van por el router magpie (`MAGPIE_URL`, por defecto `http://127.0.0.1:3425/v1`). Cada experto define su modelo en el frontmatter. `OBSESSION_MAIN` y `OBSESSION_VERIFIER` cambian el planificador y el verificador.
- Para agregar un MCP externo: añade una entrada en `mcp.json` y nómbrala en el `mcp:` de un experto. Si devuelve listings con la forma `store, title, price_pen, url, seller, rating, sold`, entra al store. Si devuelve otra cosa, el texto se trunca.
- `prompts/` y `prompts/expert_harness.md` se compilan dentro del binario: después de editarlos hay que recompilar. `experts/` y `skills/` se leen al ejecutar.
