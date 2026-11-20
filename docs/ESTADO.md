# Estado de Obsession (2026-11-20)

Obsession es un harness propio para tomar decisiones de compra. Su diferenciador es la obsesión acotada: busca y verifica hasta tener evidencia, dentro de un presupuesto de pasos, rondas y tokens. El primer dominio es compras en Perú.

## Qué funciona hoy (medido)

| Pieza | Evidencia |
|---|---|
| Búsqueda con expertos agentes, recomprobación de páginas, verificador de otra familia de modelo y ranker con rondas | `obsession eval` (10 casos): 10/10 sin error, 9/10 en menos de 5 min, 1 pick falso de 47, ~180 mil tokens por caso |
| 10 tiendas: Falabella y 7 VTEX (Plaza Vea, Oechsle, Promart, Coolbox, Estilos, Wong, Metro) más Amazon y AliExpress | Las VTEX responden en vivo; Amazon y AliExpress bloquean con anti-bot y se reportan como bloqueo, no se evaden |
| Paquetes de expertos (`packages/`, `obsession install`) con requisitos y sesiones | Tests de validación (rutas, nombres), merge con choques y desinstalación |
| Conexiones (Temu vía AnyWeb con tu sesión) | Obsession no ve contraseñas; solo comprueba el perfil del navegador |
| BYOK (`byok:<proveedor>/<modelo>`, claves con DPAPI) y tope de tokens por corrida | Tests de cifrado, ruteo y URLs; no se probó con un proveedor real |
| Negociación copiloto (`obsession negotiate`) con guardas G1-G5 en código | Simulador, 8 vendedores: 0 violaciones de límite, 0 compromisos sin checkpoint, 0 fugas de datos, 2/2 estafas detectadas, captura de margen 0.94, cierre 1 % bajo el objetivo |
| App de escritorio (Tauri) | Muestra cada paso de la búsqueda y el panel de Conexiones |
| CI en GitHub Actions (Windows) | Tests y build release en cada push |

## Qué depende de ti

1. **Juez del eval de compras:** marcar mejor, igual o peor por caso en `eval/results/<fecha>.md`. Es el criterio de "mejor compra" que no se puede automatizar. La meta es ≥ 7/10.
2. **Tus reglas de negociación (R1-R10)** en `packages/core/skills/negotiation/SKILL.md`: lo que harías si no te diera pena regatear.
3. **Clave de afiliado de AliExpress** (AliExpress Portals y Open Platform): es la única vía oficial y fiable para AliExpress. No la pegues en el chat; guárdala con `obsession provider`/`key` cuando exista el conector.
4. **Aprobar en AnyWeb** las tools de cada sitio nuevo (`review`). Por diseño, esa aprobación es humana.

## Límites conocidos

- Temu y AliExpress no se vuelven fiables con un modelo más inteligente: bloquean el acceso sin navegador. Las vías son tu sesión (Temu) o la API de afiliados (AliExpress).
- AliExpress se recomprueba buscando de nuevo, no abriendo la página, porque la página necesita JavaScript. Su precio cambia entre búsquedas, y eso explica el único pick falso.
- Negociación: el tono promedia 3.25/5 (el agente repite frases) y queda 1 falsa alarma causada por el vendedor simulado. Nada se probó con vendedores reales.
- La app lee `packages/` del repo donde se compiló. Para otra PC hay que empaquetarlos como recursos.

## Siguientes pasos, en orden

1. **AnyWeb compila Hiraoka y Ripley** (HTML limpio, sin anti-bot) en modo HTTP; los empaquetas con `{package}` y entran como expertos. Es la primera prueba completa de la fábrica de MCPs.
2. **Conector de AliExpress por API oficial**, cuando tengas la clave.
3. **Panel de negociación en la app** (paso 4 del diseño) y tono menos repetitivo.
4. **Un segundo dominio** (elegir stack de software), para demostrar que el harness es general.
5. **Acompañante y SaaS**, solo con usuarios reales.

## Cómo verificar

```
cargo test
obsession eval              # compras, ~30 min
obsession negotiate eval    # negociación en simulador, ~30 min
```
