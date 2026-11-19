"use strict";

const $ = (id) => document.getElementById(id);
const tauri = window.__TAURI__;

// Verifier flags in words a buyer understands.
const FLAGS = {
  accesorio: ["Accesorio", "No es el producto en sí: un cable, una funda, una almohadilla."],
  precio_sospechoso: ["Precio sospechoso", "Mucho más barato que el precio justo: puede ser réplica u otra versión."],
  posible_falsificacion: ["Posible réplica", "Hay señales de falsificación en el título, el vendedor o el precio."],
  importacion: ["Importado", "Llega del extranjero: demora más y puede pagar impuestos."],
  costo_estimado: ["Costo estimado", "Envío e impuestos estimados por el modelo, no calculados."],
  sin_vendedor: ["Sin vendedor", "La tienda no muestra quién vende."],
  pocas_resenas: ["Pocas reseñas", "Hay poca gente que lo haya comprado y opinado."],
  otra_categoria: ["Otra categoría", "Es un tipo de producto distinto al que pediste."],
};
const PHASES = ["plan", "search", "check", "verify", "rank"];

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "text") node.textContent = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
    else node.setAttribute(k, v);
  }
  for (const c of children) if (c != null) node.append(c);
  return node;
}

const soles = (n) => (typeof n === "number" ? `S/ ${n.toFixed(2)}` : "—");

function setPhase(phase) {
  const at = PHASES.indexOf(phase);
  document.querySelectorAll("#steps li").forEach((li, i) => {
    li.classList.toggle("done", i < at);
    li.classList.toggle("active", i === at);
  });
}

function reset() {
  for (const id of ["result", "dossier", "experts-card", "checks-card", "log-card", "error"]) $(id).hidden = true;
  for (const id of ["picks", "dossier-body", "experts", "checks", "log"]) $(id).replaceChildren();
  $("steps").hidden = false;
  setPhase("plan");
}

function list(items, fmt = (x) => x) {
  if (!Array.isArray(items) || !items.length) return document.createTextNode("—");
  return el("ul", {}, ...items.map((x) => el("li", { text: fmt(x) })));
}

function showDossier(d) {
  const range = d.fair_price_pen ? `${soles(d.fair_price_pen.min)} a ${soles(d.fair_price_pen.max)}` : "—";
  const rows = [
    ["Necesidad real", document.createTextNode(d.need || "—")],
    ["Presupuesto", document.createTextNode(d.budget_pen ? soles(d.budget_pen) : "sin límite claro")],
    ["Precio justo estimado", document.createTextNode(range)],
    ["Candidatos", list(d.candidates, (c) => `${c.model}: ${c.why}`)],
    ["Otras categorías que valen la pena", list(d.category_jumps)],
    ["Qué vigilar", list(d.defects_to_watch)],
  ];
  $("dossier-body").replaceChildren(...rows.flatMap(([t, v]) => [el("dt", { text: t }), el("dd", {}, v)]));
  $("dossier").hidden = false;
}

const experts = new Map();
function expertBox(name) {
  if (!experts.has(name)) {
    const box = el("div", { class: "expert" },
      el("h3", {}, name, el("span", { class: "state", text: "buscando…" })),
      el("ul"));
    experts.set(name, box);
    $("experts").append(box);
    $("experts-card").hidden = false;
  }
  return experts.get(name);
}

function trust(score) {
  const n = Number(score) || 0;
  const [label, color] = n >= 75 ? ["confianza alta", "var(--ok)"] : n >= 50 ? ["confianza media", "var(--warn)"] : ["confianza baja", "var(--bad)"];
  return el("div", { class: "trust" },
    el("div", { class: "bar" }, el("i", { style: `width:${Math.min(n, 100)}%;background:${color}` })),
    el("small", { text: `${n}/100 · ${label}` }));
}

function openUrl(url) {
  if (tauri?.opener) tauri.opener.openUrl(url);
  else window.open(url, "_blank");
}

function showReport(r) {
  setPhase("done");
  document.querySelectorAll("#steps li").forEach((li) => { li.classList.remove("active"); li.classList.add("done"); });
  $("verdict").textContent = r.verdict || "No hubo veredicto.";
  const cards = (r.picks || []).map((p) => {
    const flags = (p.flags || []).map((f) => {
      const [name, help] = FLAGS[f] || [f, ""];
      return el("span", { class: "flag", title: help, text: name });
    });
    const checked = p.check === "search_recheck"
      ? "Confirmado al volver a buscarlo (la página de AliExpress no se puede leer sin navegador)."
      : `Confirmado en la página${p.in_stock ? ", con stock" : ""}${p.seller ? ` · vende ${p.seller}` : ""}.`;
    return el("article", { class: "pick" },
      el("div", { class: "rank", text: `${p.rank}` }),
      el("div", {},
        el("span", { class: "store", text: p.store }),
        el("h3", { text: p.title }),
        el("p", { class: "why", text: p.why || "" }),
        el("p", { class: "note", text: `${checked} ${p.note || ""}` }),
        el("div", { class: "flags" }, ...flags)),
      el("div", { class: "side" },
        el("div", { class: "price", text: soles(p.price_pen) }),
        el("div", { class: "cost", text: p.true_cost_pen ? `costo real ≈ ${soles(p.true_cost_pen)}` : "" }),
        trust(p.trust),
        el("button", { class: "link", text: "Abrir en la tienda", onclick: () => openUrl(p.url) })));
  });
  if (!cards.length) cards.push(el("p", { class: "muted", text: "Ninguna opción pasó la verificación." }));
  $("picks").replaceChildren(...cards);
  if (r.rejected?.length) {
    $("picks").append(el("details", { class: "rejected" },
      el("summary", { text: `Descartadas al revisar (${r.rejected.length})` }),
      el("ul", {}, ...r.rejected.map((x) => el("li", { text: `[${x.store}] ${x.title}: ${x.reason}` })))));
  }
  const min = Math.floor((r.elapsed_s || 0) / 60), sec = (r.elapsed_s || 0) % 60;
  $("stats").textContent = `${r.seen} productos vistos · ${r.verified} verificados · ${r.rounds} ronda(s) · ${min} min ${sec} s`;
  $("result").hidden = false;
  $("result").scrollIntoView({ behavior: "smooth", block: "start" });
  $("go").disabled = false;
  $("go").textContent = "Buscar";
}

function warn(text) {
  $("log").append(el("li", { text }));
  $("log-card").hidden = false;
}

function onEvent(e) {
  switch (e.kind) {
    case "phase": setPhase(e.phase); break;
    case "plan": showDossier(e.dossier); break;
    case "round": setPhase("search"); if (e.round > 1) warn(`Ronda ${e.round}: ${e.hint}`); break;
    case "search": {
      const box = expertBox(e.expert);
      box.querySelector("ul").append(el("li", {}, `«${e.query}»`, el("span", { class: "n", text: `${e.results}` })));
      break;
    }
    case "expert_note": warn(`${e.expert}: ${e.text}`); break;
    case "expert_done": {
      const box = expertBox(e.expert);
      box.classList.add("done");
      box.querySelector(".state").textContent = `entregó ${e.picks}`;
      if (e.missing) box.append(el("p", { class: "missing", text: `No encontró: ${e.missing}` }));
      break;
    }
    case "check":
      $("checks").append(el("li", {},
        el("span", { class: e.ok ? "ok" : "no", text: e.ok ? "✓" : "✗" }),
        el("span", { class: "store", text: e.store }),
        el("span", { text: e.title }),
        el("span", { class: "muted", text: e.ok ? soles(e.page_price ?? e.listing_price) : e.note })));
      $("checks-card").hidden = false;
      break;
    case "not_satisfied": warn(`No quedó conforme y busca otra vez: ${e.hint}`); break;
    case "warning": warn(e.text); break;
    case "done": showReport(e.report); break;
    case "error":
      $("error").textContent = e.text;
      $("error").hidden = false;
      $("go").disabled = false;
      $("go").textContent = "Buscar";
      break;
  }
}

async function start() {
  const query = $("query").value.trim();
  reset();
  experts.clear();
  $("go").disabled = true;
  $("go").textContent = "Buscando…";
  try {
    if (tauri) await tauri.core.invoke("search", { query });
    else await replay(); // outside the app: replays a recorded run so the UI can be checked in a browser
  } catch (err) {
    onEvent({ kind: "error", text: String(err) });
  }
}

async function replay() {
  const lines = (await (await fetch("demo-events.jsonl")).text()).trim().split("\n");
  for (const line of lines) {
    onEvent(JSON.parse(line));
    await new Promise((r) => setTimeout(r, 40));
  }
}

if (tauri) tauri.event.listen("obsession", (msg) => onEvent(msg.payload));
$("go").addEventListener("click", start);
$("query").addEventListener("keydown", (e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) start(); });
document.querySelectorAll(".chip").forEach((c) => c.addEventListener("click", () => { $("query").value = c.dataset.q; $("query").focus(); }));
