# Paquetes de expertos

Un paquete agrupa expertos, skills y servidores MCP. Es una carpeta con un `obsession-package.json`:

```json
{
  "name": "temu-anyweb",
  "version": "0.1.0",
  "description": "Temu Perú con la sesión del usuario, a través del MCP de AnyWeb.",
  "source": "anyweb",
  "experts": ["experts/temu.md"],
  "skills": [],
  "mcp_servers": {
    "temu": {"command": "node", "args": ["…/entry.mjs"], "login": {"label": "Temu", "site": "https://www.temu.com", "profile": "temu-chrome", "command": "node", "args": ["…/cli.mjs", "temu", "login"]}}
  },
  "requires": [
    {"kind": "binary", "name": "node", "check": ">=24", "why": "el MCP de AnyWeb corre en Node 24"},
    {"kind": "binary", "name": "chrome", "check": "C:/Program Files/Google/Chrome/Application/chrome.exe", "why": "AnyWeb abre Temu en Chrome"},
    {"kind": "session", "name": "temu", "why": "Temu solo muestra precios con tu cuenta"}
  ],
  "tested_with": ["commandcode-plan/deepseek/deepseek-v4.1-flash"]
}
```

| Campo | Obligatorio | Qué es |
|---|---|---|
| `name` | sí | Nombre del paquete y de su carpeta. Solo `a-z`, `0-9`, `-` y `_`. |
| `version`, `description`, `source` | sí | `source` dice de dónde viene: `builtin`, `anyweb`, … |
| `experts` | no | Rutas a los `.md` de expertos, relativas al paquete. |
| `skills` | no | Rutas a carpetas con `SKILL.md`. |
| `mcp_servers` | no | Igual que el antiguo `mcp.json`: `command` (no vacío; `self` es el propio binario), `args` y `login` opcional. |
| `requires` | no | `binary`: un programa en el PATH. `check` puede ser `>=N` (versión mayor de `<name> --version`) o la ruta del archivo cuando no está en el PATH. `session`: el `login` de un MCP del paquete. |
| `tested_with` | no | Modelos con los que se evaluó. |

Reglas: las rutas son relativas y sin `..`, `.`, raíz ni unidad. Un experto solo puede usar MCP de su propio paquete; las skills pueden venir de otro (las compartidas están en `core`). Campos desconocidos se rechazan.

## Dónde viven

- De serie: `packages/` del repo (`core`, `tiendas-pe`, `tiendas-intl`, `temu-anyweb`).
- Instalados por ti: `%LOCALAPPDATA%\Obsession\packages\`.

Se cargan los dos. Si un paquete repite el nombre de un paquete, experto, skill o MCP ya cargado, no se carga y sale un aviso; nunca reemplaza al primero. Los expertos de un paquete con requisitos sin cumplir no participan en la búsqueda, y la app avisa por qué.

## Comandos

```
obsession packages              # instalados, con lo que les falta
obsession install <carpeta>     # valida, copia y dice qué falta (p. ej. obsession login temu)
obsession uninstall <paquete>   # solo los instalados por ti
```
