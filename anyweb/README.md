# Tiendas peruanas vía AnyWeb

Declaraciones de AnyWeb para que Obsession consulte tiendas como expertos. Cada tool
`search_products` recibe `{"query": "..."}` y devuelve `{"data":{"items":[{title, price, url}]}}`,
el formato que lee Obsession (ver `docs/packages.md`, "MCPs generados por AnyWeb").

| Sitio | Declaración | Estado |
|---|---|---|
| Hiraoka | `sites/hiraoka.json` | `propose` pasó (2 train: `sony`, `jbl`; eval: `samsung`). Falta `review` y `package`. |
| Ripley | `sites/ripley.json` | Bloqueado. No hay receta. |

## Hiraoka: lo que debes ejecutar

Desde el checkout de AnyWeb (`C:\Users\akimi\Desktop\anyweb`), en PowerShell.
Hiraoka responde con ~21 KB de cabeceras HTTP y Node corta en 16 KB (`UND_ERR_HEADERS_OVERFLOW`,
que AnyWeb reporta como `UPSTREAM_UNAVAILABLE`). Por eso todos los comandos llevan el límite ampliado:

```powershell
$env:NODE_OPTIONS = "--max-http-header-size=65536"
node src/cli.mjs review --site https://hiraoka.com.pe
node src/cli.mjs package --site https://hiraoka.com.pe
```

En `review`, escribe una búsqueda (por ejemplo `xiaomi`). Antes de aprobar con `a`, comprueba:

- Solo aparece `GET https://hiraoka.com.pe/gpsearch/ (html)` con el parámetro `q`.
- La vista previa trae títulos, `price` en soles sin símbolo (p. ej. `"3499"`) y URLs de `hiraoka.com.pe`.
- Los precios coinciden con la página abierta a mano en el navegador.

El MCP generado también necesita el límite: en el `mcp_servers` del paquete de Obsession usa
`"args": ["--max-http-header-size=65536", "{package}/<bundle>.mjs"]`, con el nombre de archivo
que devuelve `package`.

### Límite de la búsqueda

AnyWeb infiere un control `contains_any`: algún resultado debe contener **la consulta completa**
(sin distinguir mayúsculas) en `title` o `url`. Si no, la tool falla con `VERIFICATION_FAILED`.
`sony` o `xiaomi` funcionan; `audifonos sony` falla, porque los títulos dicen "Sony Audífonos …".
El experto de Obsession debe buscar con una palabra que aparezca literal (marca o modelo).

## Ripley: por qué está bloqueado

1. El término de búsqueda va en la ruta (`/search/<término>`) y no hay variante por parámetro
   (`/search?q=` y `/search?term=` dan 404). Las recetas HTTP de AnyWeb solo parametrizan la query
   y exigen la misma ruta en todas las demostraciones.
2. El Chromium headless de `explore` recibió la página del WAF "¡Alto, no puedes acceder!" en las
   tres demostraciones. `propose` respondió `NO_REPLAYABLE_REQUESTS`. No se intentó saltar el bloqueo.

Los selectores de `sites/ripley.json` (`a.product-link`, `.product-item--name`,
`.product-price-price`) se comprobaron con el extractor de AnyWeb sobre una página real, pero no
bastan. Ripley necesita otra vía, por ejemplo un conector específico como el de Temu.
