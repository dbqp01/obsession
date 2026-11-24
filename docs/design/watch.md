# watch: el modo "siempre encendido" de Obsession, diseño v0

Estado: diseño. No hay código todavía. Este documento define qué construir, dónde va dentro del harness actual y cómo se evalúa sin esperar a que baje un precio de verdad.

## 1. Objetivo y no-objetivos

**Objetivo.** Que una compra recomendada siga vigiéndose después de la corrida. El veredicto de hoy es cierto ahora; el watch es la pieza que lo mantiene cierto (o avisa cuando dejó de serlo) hasta que el humano paga. Esto es lo que Muse resuelve con su VM en la nube y Obsession no tiene: cuando la app se cierra, no existe nada.

El watch no es una feature nueva: es el mismo harness en modo barato. Reusa el paso 3 del flujo (reapertura de página, `stores::PageCheck`) como sondeo. Nada de expertos ni verificador en cada sondeo.

**v0 incluye:**
- Watch por pick: `obsession watch add <run> <rank> --below <precio>`.
- Sondeos baratos (un GET por pick) con estado persistente en `obsession.db`, mismo patrón que `negotiations`: sobrevive cerrar la app.
- Degradación honesta: `sin cambios` y `no pude verificar` son estados distintos, siempre. Un sondeo fallido nunca se reporta como silencio.
- Notificación toast de Windows cuando algo cambia, con enlace a la página verificada en vivo.
- Instalación del despertador con el programador de tareas de Windows y `obsession watch doctor` para comprobar que sigue vivo.

**v0 no incluye:**
- Correr el sondeo desde la nube (VPS). Todos los GETs del sondeo son de tiendas que no piden sesión (VTEX, JSON-LD, Knasta); solo Temu la necesita. Un poller remoto es una decisión aparte, después de que funcione en local.
- Watch sobre la búsqueda completa. Un watch vigila un pick; vigilar "la mejor compra" otra vez es una corrida, no un sondeo.
- Compra automática al cruzar el umbral. Como en negociación: el humano paga, siempre.
- Watch de listings de marketplace (depende de la fuente que falta para cerrar el círculo con el negociador).

## 2. Arquitectura dentro del harness

### 2.1 Qué es un watch

Una fila en `obsession.db` que apunta al pick de una corrida (por `listing_url`, no por id de corrida, para que sobreviva a reinstalar paquetes), con su umbral, su intervalo y su estado. El sondeo es una función pura del harness: dado un `Listing`, produce un `PageCheck` con el código exacto del paso 3 de `run_search` (página para las tiendas que lo permiten, `recheck_by_search` para AliExpress, con `method` en el resultado).

No va en `experts/` ni usa el planificador. Su definición vive en `src/watch.rs` (nuevo), reutiliza `stores::PageCheck` y sigue el patrón de turno de `negotiate.rs`: nada de procesos vivos esperando; algo externo (el programador de tareas o la app abierta) ejecuta `obsession watch run-due` y el binario hace su pasada y sale.

### 2.2 Tablas (mismo patrón de `db::open`, `CREATE TABLE IF NOT EXISTS`)

```sql
CREATE TABLE IF NOT EXISTS watches (
  id INTEGER PRIMARY KEY, run_id INTEGER REFERENCES runs(id), listing_url TEXT NOT NULL,
  store TEXT NOT NULL, title TEXT NOT NULL,
  below_pen REAL,               -- umbral; NULL = solo avisar si deja de confirmar
  interval_min INTEGER NOT NULL DEFAULT 180,
  status TEXT NOT NULL,         -- ok | stale | triggered | paused
  last_checked INTEGER, last_price_pen REAL, consecutive_failures INTEGER DEFAULT 0,
  created INTEGER DEFAULT (unixepoch()), updated INTEGER);
CREATE TABLE IF NOT EXISTS watch_checks (
  watch_id INTEGER REFERENCES watches(id), ts INTEGER NOT NULL,
  outcome TEXT NOT NULL,        -- confirmed | changed | unconfirmed | error
  price_pen REAL, method TEXT, note TEXT);
```

Estados y transiciones:

| De | Evento | A |
|---|---|---|
| `ok` | precio ≤ umbral, o pick que confirmaba deja de confirmar | `triggered` + notificación |
| `ok` | sondeo bien, sin cruce | `ok` |
| cualquiera | 1-2 fallos seguidos | mismo estado, `consecutive_failures` sube |
| cualquiera | 3+ fallos seguidos | `stale` + un solo aviso ("no pude verificar N veces") |
| `stale` | un sondeo vuelve a confirmar | `ok` (y si cruza umbral, `triggered`) |
| `triggered` | el humano hace clic o `watch snooze` | `paused` (la condición sigue; rearmar es manual) |

Regla de la marca: "sin cambios" es solo el resultado `confirmed` con precio igual. `error` siempre deja nota del motivo (timeout, anti-bot, 404), nunca vacía.

### 2.3 Disparo y notificación

- **Quien despierta:** el programador de tareas de Windows ejecuta `obsession watch run-due` cada 30 min (comando de instalación impreso por `obsession watch doctor --install`, con `schtasks /create`). Si la app está abierta, su loop también puede llamar al mismo subcomando; es idempotente (solo sondea lo vencido).
- **`run-due`** sondea cada watch con `status != paused` y `last_checked` vencido, escribe `watch_checks`, actualiza `watches`, y para cada transición a `triggered` o `stale` imprime una línea JSON y dispara el toast.
- **Toast v0:** PowerShell inline con `Windows.UI.Notifications` (sin módulos externos); título = producto, línea = precio nuevo y tienda, clic abre la URL. Si falla, la línea queda en stdout y en la app: la notificación no es la única superficie. Sin toasts de 22:00 a 08:00 (hora de Lima), igual que los límites de mensajes del negociador; el aviso queda registrado y sale al despertar.
- **`watch doctor`:** comprueba que la tarea programada existe y que `max(last_checked)` no lleva más de 3× el intervalo mayor vencido. Si el poller murió, los watches pasan a `stale` con nota "poller muerto" — el watch vigila también al vigilante.

### 2.4 Costo

Un sondeo es 1 GET por watch vencido (o 1 búsqueda fresca en AliExpress). Con el intervalo por defecto de 3 h, un watch cuesta ~8 GETs al día. Sin LLM. El harness completo (expertos + verificador, ~180 mil tokens por corrida) solo corre cuando el humano lo pide, nunca dentro del sondeo.

## 3. CLI y app

| Comando | Qué hace |
|---|---|
| `obsession watch add <run> <rank> [--below <precio>] [--every <min>]` | Crea el watch desde el ranking guardado. Sin `--below`, vigila solo que el pick siga confirmado. |
| `obsession watch list` | Tabla: id, producto corto, tienda, umbral, estado, último precio, hace cuánto se chequeó. |
| `obsession watch run-due` | Una pasada de sondeos vencidos; imprime JSON por transición. |
| `obsession watch snooze <id> | pause <id> | remove <id>` | Silencia, pausa o borra. |
| `obsession watch doctor [--install]` | Estado del poller y de cada watch; `--install` crea la tarea programada. |

En la app: comandos Tauri `list_watches`, `add_watch`, `run_watch_due`, `answer_watch` (snooze/pause/remove) y un panel **Watches** con la lista, el estado con color (ok/stale/triggered) y el botón **Vigilar** en cada pick del veredicto. Eventos `Emit` nuevos: `watch_update`, `watch_alert`.

## 4. Evaluación

Un vendedor guionizado no aplica aquí; el equivalente es un **servidor HTTP de prueba** que responde `PageCheck` guionizados (secuencias fijas de precio/404/timeout), igual que el vendedor guionizado de negociación corre en `cargo test` sin red.

Casos fijos en `tests/watch.rs`:

| Caso | Secuencia guionizada | Resultado correcto |
|---|---|---|
| `cruce` | 1020, 1000, 840 | `triggered` exactamente en la 3ª pasada; toast con 840 |
| `se_vendio` | ok, ok, 404 | `triggered` con motivo "dejó de confirmar", no confundido con precio |
| `rancio` | ok, timeout, timeout, timeout | `stale` tras el 3er fallo; `unconfirmed` nunca reportado como "sin cambios" |
| `se_recompone` | stale, ok | vuelve a `ok`; si además cruza, `triggered` |
| `reinicia` | matar el proceso entre checks, reabrir y sondear | estado idéntico al previo (persistencia) |
| `poller_muerto` | `last_checked` viejo + doctor | `stale` con nota "poller muerto" |
| `silencio` | toast fuera de horario | sin toast, aviso registrado y diferido |

Metas del reporte (`eval/results/watch-<stamp>.md` con las mismas convenciones que el eval de compra):

| Campo | Qué mide | Meta |
|---|---|---|
| `survives_restart` | el estado sobrevive cerrar y reabrir | 1 |
| `fires_in_window` | dispara dentro de su intervalo | 1 |
| `stale_never_silent` | fallos nunca marcados "sin cambios" | 0 violaciones |
| `alert_is_live` | el precio de la alerta coincide con la página al abrirla (±2 %) | 100 % |
| `quiet_hours_respected` | toasts fuera de horario | 0 |
| `checks_per_day` | costo por watch | ≤ 12 GETs |

## 5. Plan de construcción

Cada paso es un PR pequeño en su propia rama.

**PR 1. Estado y semántica.** Tablas en `db.rs`, `src/watch.rs` con el struct, las transiciones de 2.2 y la validación del umbral.
*Listo cuando:* `cargo test` cubre cada transición de la tabla con un caso, un reinicio conserva el estado, y `stale` tras 3 fallos es imposible de confundir con "sin cambios".

**PR 2. Sondeo y CLI.** `watch add/list/run-due/snooze/pause/remove/doctor`, sondeo reusando `PageCheck`, JSON por transición, toast y `--install` con `schtasks`.
*Listo cuando:* un watch real sobre un pick de una corrida guardada sobrevive cerrar la terminal, dispara contra el servidor de prueba, y `doctor` detecta un poller borrado.

**PR 3. Panel en la app.** Comandos Tauri y panel **Watches** con botón **Vigilar** en cada pick.
*Listo cuando:* desde la app se crea un watch, la app cerrada el sondeo sigue (por tarea programada) y la alerta abre la página.

**PR 4 (después, decisión aparte).** Poller remoto para tener los watches vivos con la PC apagada, y watch sobre la fuente marketplace cuando exista, para encadenar watch → negociación.

## 6. Preguntas abiertas

- ¿Un watch muere solo? Por defecto: tras 3 alertas `triggered` sin que el humano reaccione, pasa a `paused` con nota (la regla de Muse de escribir la condición de parada al crear el watch; aquí en código, no en el prompt).
- ¿Se puede vigilar AliExpress? Su sondeo es `recheck_by_search`, menos fiable (explica el único pick falso del eval). v0 lo permite marcando `method` en cada check; si el ruido molesta, se restringe a tiendas de página.
- ¿Notificar también cuando el precio baja sin cruzar el umbral (récord desde que se creó el watch)? Por defecto no: es el ruido que entrena a ignorar el canal.
