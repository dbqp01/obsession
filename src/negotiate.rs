//! The buyer-side negotiator: guards, approvals and message limits (code, not prompt), and the turn loop.
//! v0 is copilot only: the agent drafts, a human sends. No code path here talks to a real platform.
use crate::db;
use regex::Regex;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub const MAX_CHARS: usize = 300;
/// A cited comparable must match a verified evidence price this closely.
const EVIDENCE_TOLERANCE: f64 = 0.02;
const LIMA_OFFSET_S: i64 = -5 * 3600; // Peru has no DST.

/// Lowercase without accents, so "Trató" and "trato" hit the same phrase.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

const NUM: &str = r"\d{1,3}(?:[.,]\d{3})+(?:[.,]\d{1,2})?|\d+(?:[.,]\d{1,2})?";
static CURRENCY: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(r"s/\.?\s*({NUM})|({NUM})\s*(?:soles|sol|lucas|luquitas|mangos|pen)\b|({NUM})\s*k\b"))
});
static BARE: LazyLock<Regex> = LazyLock::new(|| re(&format!(r"\b({NUM})\b")));
static MIL: LazyLock<Regex> = LazyLock::new(|| re(r"\bmil\b"));

fn parse_num(s: &str) -> Option<f64> {
    if re(r"^\d{1,3}(?:[.,]\d{3})+").is_match(s) && !re(r"[.,]\d{1,2}$").is_match(s) {
        return s.replace(['.', ','], "").parse().ok();
    }
    if let Some((int, dec)) = s.rsplit_once([',', '.']).filter(|(_, d)| d.len() <= 2) {
        return format!("{}.{dec}", int.replace(['.', ','], "")).parse().ok();
    }
    s.parse().ok()
}

/// Every price the text states, in soles. `bare` also counts loose numbers (counters, closings).
/// shortcut: spelled-out numbers ("novecientos") are not read and "mil ..." counts as 1000; add a
/// number-word parser if the eval shows drafts writing prices in words.
pub fn prices(text: &str, bare: bool) -> Vec<f64> {
    let t = fold(text);
    let mut out: Vec<f64> = CURRENCY
        .captures_iter(&t)
        .filter_map(|c| {
            let k = c.get(3).is_some();
            let n = parse_num(c.get(1).or(c.get(2)).or(c.get(3))?.as_str())?;
            Some(if k { n * 1000.0 } else { n })
        })
        .collect();
    if bare {
        out.extend(BARE.captures_iter(&t).filter_map(|c| parse_num(&c[1])).filter(|n| *n >= 50.0));
    }
    if MIL.is_match(&t) {
        out.push(1000.0);
    }
    out
}

/// One decision the human approved, with the exact terms it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct Approval {
    pub kind: String,
    pub price_pen: Option<f64>,
    pub terms: String,
    /// The seller's offer when it was approved: if the seller moves, the approval lapses.
    pub seller_offer: Option<f64>,
}

impl Approval {
    pub fn from_meta(m: &Value) -> Approval {
        Approval {
            kind: m["kind"].as_str().unwrap_or("").into(),
            price_pen: m["price_pen"].as_f64(),
            terms: m["terms"].as_str().unwrap_or("").into(),
            seller_offer: m["seller_offer"].as_f64(),
        }
    }

    pub fn meta(&self) -> Value {
        json!({"kind": self.kind, "price_pen": self.price_pen, "terms": self.terms, "seller_offer": self.seller_offer})
    }

    pub fn live(&self, seller_offer: Option<f64>) -> bool {
        self.seller_offer == seller_offer
    }
}

/// A guard that stopped a draft and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub guard: &'static str,
    pub reason: String,
}

fn block(guard: &'static str, reason: impl Into<String>) -> Block {
    Block { guard, reason: reason.into() }
}

// shortcut: G2 and G4 are phrase lists and regexes, they miss paraphrases; move to a classifier
// when the eval shows leaks.
static CLOSING: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b(trato hecho|lo compro|te lo compro|me lo llevo|me lo quedo|me quedo con|lo tomo|cerramos|cerrado|acepto|aceptado|quedamos en|hecho el trato)\b")
});
static ADVANCE: LazyLock<Regex> = LazyLock::new(|| re(r"\b(adelanto|adelantar|adelantarte|separ\w*|apart\w*)\b"));
static PAYMENT: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b(deposit\w*|yape\w*|yapeo|plin\w*|transfer\w*|te (envio|mando|paso|hago llegar) (la |el )?(plata|dinero|billete|lucas|pago))\b")
});
static DIGIT_RUN: LazyLock<Regex> = LazyLock::new(|| re(r"\d(?:[ \-.]?\d){7,}"));
static EMAIL: LazyLock<Regex> = LazyLock::new(|| re(r"[\w.+-]+@[\w-]+\.[\w.]+|\barroba\b"));
// shortcut: an address needs a street word followed by a number; a landmark meeting point passes.
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b(av|avenida|jr|jiron|calle|pasaje|psje|mz|manzana|urb|urbanizacion)\.?\s+[\p{L} ]{2,25}\s?\d{1,4}\b")
});
static SENTENCE: LazyLock<Regex> = LazyLock::new(|| re(r"[!?¿¡\n;]|\.\s"));
static COMPARABLE: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b(vi|visto|vimos|encontre|encontrado|hay otros?|otros? iguales|otras? tiendas?|venden|lo dan|estan a|sale a|salen a|cuestan|mercado ?libre|marketplace|falabella|ripley|amazon|oechsle|coolbox|en internet|en otro lado|por ahi)\b")
});
static INVENTED_OFFER: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\b(me (ofrecieron|ofrecen|ofrece|lo dejan|lo deja|lo dejaron)|tengo otra oferta|otra oferta|otro vendedor me|otra persona me|otro comprador)\b")
});
static LINK: LazyLock<Regex> = LazyLock::new(|| {
    re(r"https?://|www\.|wa\.me|bit\.ly|\b[a-z0-9-]+\.(com|pe|net|org|io|ly|me|app|link|shop)\b")
});
static CODE_WORD: LazyLock<Regex> = LazyLock::new(|| re(r"\b(codigo|codigos|pin|otp|clave|token|numeritos|digitos)\b"));
static CODE_CONTEXT: LazyLock<Regex> =
    LazyLock::new(|| re(r"lleg|sms|verific|whatsapp|wsp|confirmaci|mensaje de texto|te mand(o|e|aron)|recib"));

/// Runs G1-G5 on one outgoing message. Empty means it may be shown or sent.
/// `owner_name` is the user's full name (from `OBSESSION_OWNER_NAME`), never sent.
pub fn check(text: &str, intent: &str, price: Option<f64>, case: &Value, approvals: &[Approval],
    owner_name: Option<&str>) -> Vec<Block> {
    let t = fold(text);
    let mut out = Vec::new();
    let bare = matches!(intent, "counter" | "conditional_accept");
    let stated: Vec<f64> = prices(text, bare).into_iter().chain(price).collect();

    // G1: never above the walk-away. A case without one blocks every price.
    let walkaway = case["walkaway_pen"].as_f64().unwrap_or(0.0);
    if let Some(p) = stated.iter().find(|p| **p > walkaway) {
        out.push(block("G1", format!("S/ {p} supera el walk-away (S/ {walkaway})")));
    }

    // G2: closing and payment wording needs a matching live approval; advances never pass.
    if ADVANCE.is_match(&t) {
        out.push(block("G2", "nunca se adelanta dinero ni se separa con pago"));
    }
    let accepts: Vec<&Approval> = approvals.iter().filter(|a| a.kind == "accept_price").collect();
    if intent == "conditional_accept" || CLOSING.is_match(&t) {
        let covered = accepts.iter().any(|a| {
            stated.iter().all(|p| a.price_pen.is_some_and(|ap| (ap - p).abs() < 0.5))
        });
        if !covered {
            out.push(block("G2", "cierre sin una aprobación accept_price vigente con ese precio"));
        }
    }
    if PAYMENT.is_match(&t) && !approvals.iter().any(|a| a.kind == "payment_method") {
        out.push(block("G2", "habla de pago sin una aprobación payment_method vigente"));
    }

    // G3: personal data.
    if DIGIT_RUN.is_match(text) {
        out.push(block("G3", "número largo (teléfono, DNI, tarjeta o cuenta)"));
    }
    if EMAIL.is_match(&t) {
        out.push(block("G3", "correo electrónico"));
    }
    if ADDRESS.is_match(&t) {
        out.push(block("G3", "dirección"));
    }
    if let Some(name) = owner_name.map(fold).filter(|n| !n.trim().is_empty()) {
        let surnames: Vec<&str> = name.split_whitespace().skip(1).filter(|w| w.len() >= 3).collect();
        if t.contains(name.trim()) || surnames.iter().any(|s| re(&format!(r"\b{}\b", regex::escape(s))).is_match(&t)) {
            out.push(block("G3", "nombre del usuario"));
        }
    }

    // G4: only comparables from the evidence; no invented offers.
    let evidence: Vec<f64> = case["evidence"].as_array().into_iter().flatten().filter_map(|e| e["price_pen"].as_f64()).collect();
    for sentence in SENTENCE.split(&t).filter(|s| COMPARABLE.is_match(s)) {
        for p in prices(sentence, true) {
            if !evidence.iter().any(|e| (e - p).abs() / e <= EVIDENCE_TOLERANCE) {
                out.push(block("G4", format!("S/ {p} citado como comparable no está en la evidencia")));
            }
        }
    }
    if INVENTED_OFFER.is_match(&t) {
        out.push(block("G4", "otra oferta u otro comprador inventado"));
    }

    // G5: shape.
    if LINK.is_match(&t) {
        out.push(block("G5", "links no permitidos"));
    }
    if text.chars().count() > MAX_CHARS {
        out.push(block("G5", format!("más de {MAX_CHARS} caracteres")));
    }
    if CODE_WORD.is_match(&t) && CODE_CONTEXT.is_match(&t) {
        out.push(block("G5", "nunca se piden ni se mandan códigos de verificación"));
    }
    out
}

fn lima(ts: i64) -> (i64, i64) {
    let local = ts + LIMA_OFFSET_S;
    (local.div_euclid(86_400), local.rem_euclid(86_400) / 3600)
}

/// Next 08:00 Lima at or after `ts`.
pub fn next_morning(ts: i64) -> i64 {
    let (day, hour) = lima(ts);
    let day = if hour >= 22 { day + 1 } else { day };
    day * 86_400 + 8 * 3600 - LIMA_OFFSET_S
}

/// Section 4.4 limits. `sent` is every message the account sent as (thread, ts); `unanswered` is how
/// many we sent in `thread` since the seller last wrote. The clock is a parameter so tests and the
/// simulator control it.
pub fn limit(sent: &[(i64, i64)], thread: i64, unanswered: usize, now: i64) -> Result<(), String> {
    let (today, hour) = lima(now);
    if !(8..22).contains(&hour) {
        return Err("horario silencioso (22:00 a 08:00, hora de Lima)".into());
    }
    let last = sent.iter().filter(|(t, _)| *t == thread).map(|(_, ts)| *ts).max();
    match (unanswered, last) {
        (0, _) => {}
        (1, Some(l)) if now - l < 24 * 3600 => return Err("espera 24 h antes del primer recordatorio".into()),
        (2, Some(l)) if now - l < 48 * 3600 => return Err("el segundo recordatorio va a las 72 h".into()),
        (n, _) if n >= 3 => return Err("ya van dos recordatorios sin respuesta".into()),
        _ => {}
    }
    if sent.iter().filter(|(_, ts)| lima(*ts).0 == today).count() >= 20 {
        return Err("tope de 20 mensajes por día en la cuenta".into());
    }
    if last.is_none() {
        let mut new_today: Vec<i64> = Vec::new();
        for (t, _) in sent {
            let first = sent.iter().filter(|(x, _)| x == t).map(|(_, ts)| *ts).min().unwrap();
            if lima(first).0 == today && !new_today.contains(t) {
                new_today.push(*t);
            }
        }
        if new_today.len() >= 5 {
            return Err("tope de 5 hilos nuevos por día".into());
        }
    }
    Ok(())
}

// ---- Turn loop -------------------------------------------------------------------------------

const PROMPT: &str = include_str!("../prompts/negotiator.md");
pub const MAX_STEPS: usize = 4;
const THREAD_TAIL: usize = 20;
/// `injection` is a note, not a scam signal: a seller who types orders at a bot is not asking for money.
const SIGNALS: [&str; 7] = ["pide_adelanto", "presion", "niega_prueba", "saca_del_chat", "historia_inconsistente", "precio_irreal", "injection"];
pub const STRONG_SIGNALS: [&str; 2] = ["pide_adelanto", "saca_del_chat"];
const INTENTS: [&str; 6] = ["opening", "question", "proof_request", "counter", "follow_up", "conditional_accept"];
const CHECKPOINTS: [&str; 5] = ["accept_price", "meeting", "payment_method", "share_contact", "continue_low_trust"];

/// One model step: (messages, tools, force a tool call) -> assistant message. Injected so tests and
/// the simulator can script it.
pub type Chat<'a> = &'a dyn Fn(&[Value], &[Value], bool) -> Result<Value, String>;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Message { text: String, intent: String, price: Option<f64> },
    Checkpoint { kind: String, price: Option<f64>, terms: String, summary: String, warnings: Vec<String> },
    WalkAway { reason: String, final_message: Option<String> },
    /// The turn ended without anything for the seller; the note is for the human.
    Nothing(String),
}

#[derive(Debug)]
pub struct Turn {
    pub outcome: Outcome,
    /// Every draft a guard stopped: (block, draft text).
    pub blocks: Vec<(Block, String)>,
}

/// True when the agent's scam signals reach the skill's threshold: one strong or two weak.
pub fn scam_flagged(signals: &[String]) -> bool {
    let scam: Vec<&String> = signals.iter().filter(|s| *s != "injection").collect();
    scam.iter().any(|s| STRONG_SIGNALS.contains(&s.as_str())) || scam.len() >= 2
}

/// Approvals the human granted that still hold for the seller's current offer.
pub fn live_approvals(thread: &[db::NegMessage], seller_offer: Option<f64>) -> Vec<Approval> {
    thread
        .iter()
        .filter(|m| m.role == "approval" && m.meta["approved"] == true)
        .map(|m| Approval::from_meta(&m.meta))
        .filter(|a| a.live(seller_offer))
        .collect()
}

fn owner_name() -> Option<String> {
    std::env::var("OBSESSION_OWNER_NAME").ok()
}

fn function(name: &str, description: &str, parameters: Value) -> Value {
    json!({"type": "function", "function": {"name": name, "description": description, "parameters": parameters}})
}

fn terminal_tools() -> Vec<Value> {
    vec![
        function("propose_message", "Redacta el siguiente mensaje al vendedor. Termina el turno.",
            json!({"type": "object", "required": ["text", "intent"], "properties": {
                "text": {"type": "string"}, "intent": {"type": "string", "enum": INTENTS},
                "price_pen": {"type": "number"}}})),
        function("request_human_checkpoint", "Pide una decisión al dueño. Termina el turno.",
            json!({"type": "object", "required": ["kind", "terms", "summary"], "properties": {
                "kind": {"type": "string", "enum": CHECKPOINTS}, "price_pen": {"type": "number"},
                "terms": {"type": "string"}, "summary": {"type": "string"},
                "warnings": {"type": "array", "items": {"type": "string"}}}})),
        function("walk_away", "Cierra la negociación sin trato. Termina el turno.",
            json!({"type": "object", "required": ["reason"], "properties": {
                "reason": {"type": "string"}, "final_message": {"type": "string"}}})),
    ]
}

fn all_tools() -> Vec<Value> {
    let mut tools = vec![
        function("read_reply", "Devuelve el hilo completo y las horas desde el último mensaje.", json!({"type": "object", "properties": {}})),
        function("update_offer", "Actualiza ofertas y señales del vendedor.",
            json!({"type": "object", "required": ["note"], "properties": {
                "our_offer_pen": {"type": "number"}, "seller_offer_pen": {"type": "number"},
                "seller_signals": {"type": "array", "items": {"type": "string", "enum": SIGNALS}},
                "note": {"type": "string"}}})),
        function("load_skill", "Carga las instrucciones completas de una skill.",
            json!({"type": "object", "required": ["name"], "properties": {"name": {"type": "string", "enum": ["negotiation"]}}})),
    ];
    tools.extend(terminal_tools());
    tools
}

/// What the model sees of the thread: no drafts, no guard logs.
fn visible(thread: &[db::NegMessage], now: i64) -> Vec<Value> {
    thread
        .iter()
        .filter(|m| matches!(m.role.as_str(), "seller" | "sent" | "checkpoint" | "approval" | "human_note"))
        .map(|m| json!({"role": m.role, "text": m.text, "hours_ago": (now - m.ts) / 3600, "meta": m.meta}))
        .collect()
}

fn hours_since_last(thread: &[db::NegMessage], now: i64) -> Option<i64> {
    thread.iter().filter(|m| m.role == "seller" || m.role == "sent").map(|m| (now - m.ts) / 3600).min()
}

/// Runs one negotiator turn. Updates `n` in place (offers, signals, concessions); the caller persists.
pub fn turn(n: &mut db::Negotiation, thread: &[db::NegMessage], event: &str, now: i64, skill: Option<&str>,
    chat: Chat, spent: &dyn Fn() -> bool) -> Turn {
    let state = json!({"status": n.status, "our_offer_pen": n.our_offer, "seller_offer_pen": n.seller_offer,
        "concessions": n.concessions, "signals": n.signals, "days_since_start": (now - n.created) / 86_400,
        "hours_since_last_message": hours_since_last(thread, now)});
    let tail = visible(thread, now);
    let tail = &tail[tail.len().saturating_sub(THREAD_TAIL)..];
    let input = json!({"event": event, "case": n.case, "state": state, "messages": tail,
        "approvals": live_approvals(thread, n.seller_offer).iter().map(Approval::meta).collect::<Vec<_>>()});
    let mut messages = vec![json!({"role": "system", "content": PROMPT}), json!({"role": "user", "content": input.to_string()})];
    let mut blocks = Vec::new();
    let owner = owner_name();
    for step in 0..MAX_STEPS {
        let last = step + 1 == MAX_STEPS || spent();
        if last {
            messages.push(json!({"role": "user", "content": "Último paso: termina el turno con propose_message, request_human_checkpoint o walk_away."}));
        }
        let offered = if last { terminal_tools() } else { all_tools() };
        let msg = match chat(&messages, &offered, last) {
            Ok(m) => m,
            Err(e) => return Turn { outcome: Outcome::Nothing(format!("el modelo falló: {e}")), blocks },
        };
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if calls.is_empty() {
            messages.push(json!({"role": "assistant", "content": msg["content"].as_str().unwrap_or("")}));
            messages.push(json!({"role": "user", "content": "Responde con una llamada a tool."}));
            continue;
        }
        messages.push(json!({"role": "assistant", "content": msg["content"].as_str().unwrap_or(""), "tool_calls": calls}));
        let mut done = None;
        for call in &calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap_or("{}")).unwrap_or(json!({}));
            let text = |k: &str| args[k].as_str().unwrap_or("").trim().to_string();
            let approvals = live_approvals(thread, n.seller_offer);
            let mut guarded = |draft: &str, intent: &str, price: Option<f64>| -> Result<(), String> {
                let found = check(draft, intent, price, &n.case, &approvals, owner.as_deref());
                if found.is_empty() {
                    return Ok(());
                }
                let why: Vec<String> = found.iter().map(|b| format!("{}: {}", b.guard, b.reason)).collect();
                blocks.extend(found.into_iter().map(|b| (b, draft.to_string())));
                Err(format!("bloqueado por los guardas ({}). Reescribe el mensaje.", why.join("; ")))
            };
            let result = if done.is_some() {
                "ignorado: el turno ya terminó".to_string()
            } else {
                match name {
                    "read_reply" => json!({"messages": visible(thread, now), "hours_since_last_message": hours_since_last(thread, now)}).to_string(),
                    "load_skill" => skill.unwrap_or("skill no disponible").to_string(),
                    "update_offer" => {
                        if let Some(p) = args["our_offer_pen"].as_f64() {
                            n.our_offer = Some(p);
                        }
                        if let Some(p) = args["seller_offer_pen"].as_f64() {
                            n.seller_offer = Some(p);
                        }
                        for s in args["seller_signals"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                            if SIGNALS.contains(&s) && !n.signals.iter().any(|x| x == s) {
                                n.signals.push(s.to_string());
                            }
                        }
                        "ok".into()
                    }
                    "propose_message" => {
                        let (draft, intent, price) = (text("text"), text("intent"), args["price_pen"].as_f64());
                        if !INTENTS.contains(&intent.as_str()) {
                            format!("intent no válido: {intent}")
                        } else if draft.is_empty() {
                            "el mensaje está vacío".into()
                        } else {
                            match guarded(&draft, &intent, price) {
                                Err(e) => e,
                                Ok(()) => {
                                    done = Some(Outcome::Message { text: draft, intent, price });
                                    "ok".into()
                                }
                            }
                        }
                    }
                    "request_human_checkpoint" => {
                        let kind = text("kind");
                        if CHECKPOINTS.contains(&kind.as_str()) {
                            done = Some(Outcome::Checkpoint {
                                kind, price: args["price_pen"].as_f64(), terms: text("terms"), summary: text("summary"),
                                warnings: args["warnings"].as_array().into_iter().flatten()
                                    .filter_map(|w| w.as_str().map(String::from)).collect(),
                            });
                            "ok".into()
                        } else {
                            format!("kind no válido: {kind}")
                        }
                    }
                    "walk_away" => {
                        let last_words = Some(text("final_message")).filter(|m| !m.is_empty());
                        match last_words.as_deref().map(|m| guarded(m, "follow_up", None)) {
                            Some(Err(e)) => e,
                            _ => {
                                done = Some(Outcome::WalkAway { reason: text("reason"), final_message: last_words });
                                "ok".into()
                            }
                        }
                    }
                    _ => format!("tool desconocida: {name}"),
                }
            };
            messages.push(json!({"role": "tool", "tool_call_id": call["id"], "content": result}));
        }
        if let Some(outcome) = done {
            if let Outcome::Message { intent, price: Some(p), .. } = &outcome {
                if matches!(intent.as_str(), "counter" | "conditional_accept") {
                    if n.our_offer.is_some_and(|o| *p > o) {
                        n.concessions += 1;
                    }
                    n.our_offer = Some(*p);
                }
            }
            return Turn { outcome, blocks };
        }
    }
    let note = if blocks.is_empty() { "el turno terminó sin decisión" } else { "el borrador siguió bloqueado por los guardas; no hay mensaje" };
    Turn { outcome: Outcome::Nothing(note.into()), blocks }
}

// ---- Copilot operations: every step persists to obsession.db, so a negotiation survives days ----

pub fn negotiator_model() -> String {
    crate::model("OBSESSION_NEGOTIATOR", &crate::main_model())
}

pub fn skill_body(root: &std::path::Path) -> Option<String> {
    crate::packages::load(root).packages.into_iter().flat_map(|p| p.skills).find(|s| s.name == "negotiation").map(|s| s.body)
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The case a run's pick proposes. Target and walk-away are proposals: the human confirms them.
pub fn case_from_pick(db: &Connection, run_id: i64, rank: usize) -> Result<Value, String> {
    let report: Option<String> = db
        .query_row("SELECT report FROM runs WHERE id = ?1", [run_id], |r| r.get(0))
        .map_err(|_| format!("no existe la corrida {run_id}"))?;
    let report: Value = report.and_then(|r| serde_json::from_str(&r).ok()).ok_or("la corrida no tiene reporte")?;
    let picks = report["picks"].as_array().cloned().unwrap_or_default();
    let pick = picks.get(rank.wrapping_sub(1)).ok_or(format!("la corrida {run_id} no tiene el pick {rank}"))?;
    let mut stmt = db
        .prepare("SELECT id, store, title, price_pen, url FROM listings WHERE run_id = ?1 AND relevant = 1 AND id != ?2")
        .map_err(|e| e.to_string())?;
    let evidence: Vec<Value> = stmt
        .query_map(rusqlite::params![run_id, pick["id"].as_i64()], |r| {
            Ok(json!({"id": format!("e{}", r.get::<_, i64>(0)?), "store": r.get::<_, String>(1)?,
                "title": r.get::<_, String>(2)?, "price_pen": r.get::<_, f64>(3)?, "url": r.get::<_, String>(4)?}))
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    let target = evidence.iter().filter_map(|e| e["price_pen"].as_f64()).fold(f64::INFINITY, f64::min);
    let budget = report["dossier"]["budget_pen"].as_f64().unwrap_or(f64::INFINITY);
    let alternative = picks.iter().filter(|p| p["id"] != pick["id"]).filter_map(|p| p["true_cost_pen"].as_f64()).fold(f64::INFINITY, f64::min);
    let walkaway = budget.min(alternative);
    Ok(json!({
        "listing": {"title": pick["title"], "url": pick["url"], "store": pick["store"], "seller": pick["seller"], "ask_pen": pick["price_pen"]},
        "evidence": evidence, "fair_price_pen": report["dossier"]["fair_price_pen"],
        "target_pen": target.is_finite().then_some(target), "walkaway_pen": walkaway.is_finite().then_some(walkaway),
        "trust": {"score": pick["trust"], "flags": pick["flags"], "note": pick["note"]}, "mode": "copilot",
    }))
}

/// Creates the negotiation with limits the human confirmed. Low trust waits for a checkpoint first.
pub fn start(db: &Connection, run_id: Option<i64>, case: &Value, now: i64) -> Result<i64, String> {
    let (Some(target), Some(walkaway)) = (case["target_pen"].as_f64(), case["walkaway_pen"].as_f64()) else {
        return Err("faltan target y walk-away confirmados".into());
    };
    if !(target > 0.0 && target <= walkaway) {
        return Err("el target debe ser positivo y no mayor que el walk-away".into());
    }
    let low = case["trust"]["score"].as_i64().is_some_and(|s| s < 40);
    let id = db::new_negotiation(db, run_id, case, if low { "awaiting_human" } else { "drafting" }, now).map_err(|e| e.to_string())?;
    if low {
        let meta = json!({"kind": "continue_low_trust", "terms": "seguir pese al trust bajo", "warnings": case["trust"]["flags"]});
        db::add_neg_message(db, id, now, "checkpoint", "El trust del vendedor es menor que 40. ¿Seguimos?", &meta).map_err(|e| e.to_string())?;
    }
    Ok(id)
}

/// Runs a turn on the stored negotiation and stores what came out of it.
pub fn advance(db: &Connection, id: i64, event: &str, now: i64, skill: Option<&str>, chat: Chat, spent: &dyn Fn() -> bool)
    -> Result<Turn, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let mut n = db::negotiation(db, id).map_err(e)?;
    if matches!(n.status.as_str(), "walked" | "paused") {
        return Err(format!("la negociación {id} está {}", n.status));
    }
    let thread = db::neg_messages(db, id).map_err(e)?;
    let t = turn(&mut n, &thread, event, now, skill, chat, spent);
    for (b, draft) in &t.blocks {
        db::add_neg_message(db, id, now, "guard_block", draft, &json!({"guard": b.guard, "reason": b.reason})).map_err(e)?;
    }
    match &t.outcome {
        Outcome::Message { text, intent, price } => {
            db::add_neg_message(db, id, now, "agent_draft", text, &json!({"intent": intent, "price_pen": price})).map_err(e)?;
            n.status = if intent == "conditional_accept" { "deal_pending_payment" } else { "waiting_seller" }.into();
        }
        Outcome::Checkpoint { kind, price, terms, summary, warnings } => {
            let meta = json!({"kind": kind, "price_pen": price, "terms": terms, "warnings": warnings});
            db::add_neg_message(db, id, now, "checkpoint", summary, &meta).map_err(e)?;
            n.status = "awaiting_human".into();
        }
        Outcome::WalkAway { reason, final_message } => {
            if let Some(m) = final_message {
                db::add_neg_message(db, id, now, "agent_draft", m, &json!({"intent": "walk_away"})).map_err(e)?;
            }
            db::add_neg_message(db, id, now, "human_note", &format!("El agente se retira: {reason}"), &json!({})).map_err(e)?;
            n.status = "walked".into();
        }
        Outcome::Nothing(note) => {
            db::add_neg_message(db, id, now, "human_note", note, &json!({})).map_err(e)?;
            n.status = "drafting".into();
        }
    }
    db::save_negotiation(db, &n, now).map_err(e)?;
    Ok(t)
}

/// The human pasted the seller's reply. Their last unsent draft is taken as sent: the human sends.
pub fn reply(db: &Connection, id: i64, text: &str, now: i64, skill: Option<&str>, chat: Chat, spent: &dyn Fn() -> bool)
    -> Result<Turn, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("la respuesta del vendedor está vacía".into());
    }
    mark_sent(db, id, now)?;
    db::add_neg_message(db, id, now, "seller", text, &json!({})).map_err(|e| e.to_string())?;
    advance(db, id, "seller_reply", now, skill, chat, spent)
}

pub fn mark_sent(db: &Connection, id: i64, now: i64) -> Result<(), String> {
    let thread = db::neg_messages(db, id).map_err(|e| e.to_string())?;
    let last = thread.iter().rev().find(|m| matches!(m.role.as_str(), "agent_draft" | "sent" | "seller"));
    if let Some(m) = last.filter(|m| m.role == "agent_draft") {
        db::add_neg_message(db, id, now, "sent", &m.text, &m.meta).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The open checkpoint, if the negotiation waits on the human.
pub fn pending_checkpoint(db: &Connection, id: i64) -> Result<Option<db::NegMessage>, String> {
    let n = db::negotiation(db, id).map_err(|e| e.to_string())?;
    if n.status != "awaiting_human" {
        return Ok(None);
    }
    Ok(db::neg_messages(db, id).map_err(|e| e.to_string())?.into_iter().rev().find(|m| m.role == "checkpoint"))
}

/// Records the human's answer to the open checkpoint. `price` lets them edit it down, never up.
pub fn decide(db: &Connection, id: i64, approved: bool, price: Option<f64>, now: i64) -> Result<Approval, String> {
    let cp = pending_checkpoint(db, id)?.ok_or("no hay un checkpoint pendiente")?;
    let n = db::negotiation(db, id).map_err(|e| e.to_string())?;
    let asked = cp.meta["price_pen"].as_f64();
    if let (Some(p), Some(a)) = (price, asked) {
        if p > a {
            return Err("al editar solo puedes bajar el precio".into());
        }
    }
    let price = price.or(asked);
    if price.is_some_and(|p| p > n.case["walkaway_pen"].as_f64().unwrap_or(0.0)) {
        return Err("ese precio supera el walk-away; subirlo es otra decisión".into());
    }
    let a = Approval { kind: cp.meta["kind"].as_str().unwrap_or("").into(), price_pen: price,
        terms: cp.meta["terms"].as_str().unwrap_or("").into(), seller_offer: n.seller_offer };
    let mut meta = a.meta();
    meta["approved"] = json!(approved);
    let text = if approved { "El dueño aprueba." } else { "El dueño rechaza." };
    db::add_neg_message(db, id, now, "approval", text, &meta).map_err(|e| e.to_string())?;
    Ok(a)
}

/// In copilot the 4.4 limits are advice: the human sends, so this only says what the limit would be.
pub fn limit_hint(db: &Connection, id: i64, now: i64) -> Result<Option<String>, String> {
    let sent: Vec<(i64, i64)> = db
        .prepare("SELECT negotiation_id, ts FROM neg_messages WHERE role = 'sent'")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .map_err(|e: rusqlite::Error| e.to_string())?;
    let thread = db::neg_messages(db, id).map_err(|e| e.to_string())?;
    let unanswered = thread.iter().rev().take_while(|m| m.role != "seller").filter(|m| m.role == "sent").count();
    Ok(limit(&sent, id, unanswered, now).err())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case() -> Value {
        json!({"walkaway_pen": 980.0, "target_pen": 900.0,
            "evidence": [{"id": "e1", "price_pen": 950.0}, {"id": "e2", "price_pen": 1020.0}]})
    }

    fn guards(text: &str, intent: &str, approvals: &[Approval]) -> Vec<&'static str> {
        check(text, intent, None, &case(), approvals, Some("Ana Quispe Mamani")).iter().map(|b| b.guard).collect()
    }

    fn accept(price: f64) -> Approval {
        Approval { kind: "accept_price".into(), price_pen: Some(price), terms: "recojo hoy".into(), seller_offer: Some(950.0) }
    }

    #[test]
    fn clean_messages_pass() {
        assert!(guards("hola, buenas, ¿todavía lo tienes?", "opening", &[]).is_empty());
        assert!(guards("¿me lo dejas en 900? recojo hoy", "counter", &[]).is_empty());
        assert!(guards("he visto otros iguales a 950 acá en Cusco", "counter", &[]).is_empty());
        assert!(guards("¿me pasas foto del número de serie?", "proof_request", &[]).is_empty());
    }

    #[test]
    fn g1_blocks_prices_above_walkaway() {
        assert_eq!(guards("¿me lo dejas en S/ 1000?", "counter", &[]), ["G1"]);
        assert_eq!(guards("¿y en 990?", "counter", &[]), ["G1"], "a loose number in a counter");
        // Reworded: thousands separator, slang, "k" and "mil".
        assert_eq!(guards("te doy 1,050 lucas", "question", &[]), ["G1"]);
        assert_eq!(guards("¿unos 1k?", "question", &[]), ["G1"]);
        assert_eq!(guards("te ofrezco mil soles", "question", &[]), ["G1"]);
        assert!(check("ok", "counter", Some(985.0), &case(), &[], None).iter().any(|b| b.guard == "G1"));
        assert_eq!(prices("S/ 1.050 o S/10.50", false), [1050.0, 10.5]);
        assert!(guards("990 se me pasa del presupuesto, gracias igual", "follow_up", &[]).is_empty(),
            "repeating the seller's ask outside a counter is not an offer");
    }

    #[test]
    fn g2_needs_matching_approval_and_never_advances() {
        assert_eq!(guards("ya, trato hecho en 900", "counter", &[]), ["G2"]);
        // Reworded closings with no listed phrase still need the approval through the intent.
        assert_eq!(guards("ya, me quedo con él a 900", "counter", &[]), ["G2"]);
        assert_eq!(guards("perfecto, nos vemos y van los 900", "conditional_accept", &[]), ["G2"]);
        assert!(guards("ya, cerramos en 900", "conditional_accept", &[accept(900.0)]).is_empty());
        assert_eq!(guards("ya, cerramos en 920", "conditional_accept", &[accept(900.0)]), ["G2"], "other price");
        assert_eq!(guards("te yapeo ahorita", "question", &[]), ["G2"]);
        assert_eq!(guards("te mando la plata ahorita", "question", &[]), ["G2"], "reworded payment");
        assert_eq!(guards("te dejo un adelanto para que me lo guardes", "question", &[accept(900.0)]), ["G2"]);
        assert_eq!(guards("sepáramelo porfa", "question", &[]), ["G2"]);
    }

    #[test]
    fn approval_lapses_when_the_seller_moves() {
        let a = accept(900.0);
        assert!(a.live(Some(950.0)));
        assert!(!a.live(Some(970.0)), "the seller changed the terms");
        assert_eq!(Approval::from_meta(&a.meta()), a);
    }

    #[test]
    fn g3_blocks_personal_data() {
        assert_eq!(guards("mi número es 987654321", "question", &[]), ["G3"]);
        assert_eq!(guards("escríbeme al 9 8 7 6 5 4 3 2 1", "question", &[]), ["G3"], "spaced digits");
        assert_eq!(guards("mi dni es 45678912", "question", &[]), ["G3"]);
        assert!(guards("ana.q@gmail.com", "question", &[]).contains(&"G3"));
        assert_eq!(guards("ana punto q arroba gmail", "question", &[]), ["G3"], "spelled email");
        assert_eq!(guards("vivo en Av. El Sol 345", "question", &[]), ["G3"]);
        assert_eq!(guards("soy ana quispe", "question", &[]), ["G3"], "the user's surname");
        assert!(guards("nos vemos en la Plaza de Armas", "question", &[]).is_empty());
    }

    #[test]
    fn g4_only_cites_evidence() {
        assert_eq!(guards("he visto otros iguales a 800", "counter", &[]), ["G4"]);
        assert_eq!(guards("por ahí lo venden a 850", "question", &[]), ["G4"], "reworded");
        assert!(guards("en Falabella sale a 1,030", "question", &[]).is_empty(), "within 2 % of e2, and not an offer");
        assert_eq!(guards("un pata me lo deja en 900", "counter", &[]), ["G4"], "invented offer");
        assert_eq!(guards("tengo otra oferta, ¿lo bajas?", "question", &[]), ["G4"]);
    }

    #[test]
    fn g5_blocks_links_length_and_codes() {
        assert_eq!(guards("mira acá https://x.pe/a", "question", &[]), ["G5"]);
        assert_eq!(guards("búscalo en tienda.com", "question", &[]), ["G5"]);
        assert_eq!(guards(&"a".repeat(301), "question", &[]), ["G5"]);
        assert_eq!(guards("pásame el código de verificación", "question", &[]), ["G5"]);
        assert_eq!(guards("dime los numeritos que te llegaron por sms", "question", &[]), ["G5"], "reworded");
    }

    #[test]
    fn limits() {
        // 2026-11-16 15:00 UTC = 10:00 Lima.
        let t0 = 1_794_841_200;
        assert!(limit(&[], 1, 0, t0).is_ok());
        assert!(limit(&[], 1, 0, t0 + 13 * 3600).unwrap_err().contains("silencioso"), "23:00 Lima");
        assert_eq!(next_morning(t0 + 13 * 3600), t0 + 22 * 3600);
        let five: Vec<(i64, i64)> = (0..5).map(|t| (t, t0 + t * 60)).collect();
        assert!(limit(&five, 9, 0, t0 + 3600).unwrap_err().contains("5 hilos"), "sixth new thread");
        assert!(limit(&five, 2, 0, t0 + 3600).is_ok(), "an old thread is not new");
        let one = [(1, t0)];
        assert!(limit(&one, 1, 1, t0 + 3600).is_err(), "reminder too soon");
        assert!(limit(&one, 1, 1, t0 + 24 * 3600).is_ok());
        assert!(limit(&one, 1, 3, t0 + 100 * 3600).is_err(), "third reminder");
        let twenty: Vec<(i64, i64)> = (0..20).map(|i| (100 + i, t0 + i * 60)).collect();
        assert!(limit(&twenty, 100, 0, t0 + 7200).unwrap_err().contains("20"));
    }

    /// A chat that answers each step with the next scripted tool call, repeating the last one.
    pub(crate) fn scripted(calls: Vec<(&'static str, Value)>) -> impl Fn(&[Value], &[Value], bool) -> Result<Value, String> {
        let step = std::sync::Mutex::new(0usize);
        move |_, _, _| {
            let mut i = step.lock().unwrap();
            let (name, args) = &calls[(*i).min(calls.len() - 1)];
            *i += 1;
            Ok(json!({"content": "", "tool_calls": [{"id": format!("c{i}"), "type": "function",
                "function": {"name": name, "arguments": args.to_string()}}]}))
        }
    }

    fn negotiation(case: Value) -> db::Negotiation {
        db::Negotiation { id: 1, run_id: None, status: "drafting".into(), case, our_offer: Some(850.0), seller_offer: Some(1000.0),
            concessions: 0, signals: vec![], created: 0 }
    }

    #[test]
    fn turn_rewrites_after_a_guard_block() {
        let chat = scripted(vec![
            ("update_offer", json!({"seller_offer_pen": 950, "seller_signals": ["presion", "inventada"], "note": "baja"})),
            ("propose_message", json!({"text": "ya, trato hecho en 900", "intent": "counter", "price_pen": 900})),
            ("propose_message", json!({"text": "¿me lo dejas en 900? recojo hoy", "intent": "counter", "price_pen": 900})),
        ]);
        let mut n = negotiation(case());
        let t = turn(&mut n, &[], "seller_reply", 0, None, &chat, &|| false);
        assert!(matches!(t.outcome, Outcome::Message { ref intent, .. } if intent == "counter"));
        assert_eq!(t.blocks.iter().map(|(b, _)| b.guard).collect::<Vec<_>>(), ["G2"]);
        assert_eq!((n.our_offer, n.seller_offer, n.concessions), (Some(900.0), Some(950.0), 1));
        assert_eq!(n.signals, ["presion"], "signals outside the vocabulary are dropped");
        assert!(!scam_flagged(&["presion".into(), "injection".into()]), "an injection note is not a scam signal");
        assert!(scam_flagged(&["presion".into(), "niega_prueba".into()]));
    }

    #[test]
    fn turn_still_blocked_on_last_step_sends_nothing() {
        let chat = scripted(vec![("propose_message", json!({"text": "te yapeo el adelanto", "intent": "question"}))]);
        let mut n = negotiation(case());
        let t = turn(&mut n, &[], "seller_reply", 0, None, &chat, &|| false);
        assert!(matches!(t.outcome, Outcome::Nothing(ref note) if note.contains("bloqueado")));
        assert_eq!(t.blocks.len(), 2 * MAX_STEPS);
    }

    #[test]
    fn copilot_resumes_from_the_db_and_needs_approval_to_close() {
        let path = std::env::temp_dir().join(format!("obsession-neg-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let path = path.to_string_lossy().to_string();
        let mut c = case();
        c["trust"] = json!({"score": 70});
        let id = {
            let db = db::open(&path).unwrap();
            let id = start(&db, None, &c, 100).unwrap();
            let chat = scripted(vec![("propose_message", json!({"text": "hola, buenas, ¿todavía lo tienes?", "intent": "opening"}))]);
            advance(&db, id, "start", 100, None, &chat, &|| false).unwrap();
            id
        };
        // Next day, a new process: the state comes back from obsession.db.
        let db = db::open(&path).unwrap();
        let chat = scripted(vec![
            ("update_offer", json!({"seller_offer_pen": 950, "note": "lo deja en 950"})),
            ("propose_message", json!({"text": "dale, cerramos en 950", "intent": "conditional_accept", "price_pen": 950})),
            ("request_human_checkpoint", json!({"kind": "accept_price", "price_pen": 950, "terms": "recojo hoy", "summary": "Aceptar 950"})),
        ]);
        reply(&db, id, "sí, te lo dejo en 950", 100 + 86_400, None, &chat, &|| false).unwrap();
        let roles: Vec<String> = db::neg_messages(&db, id).unwrap().into_iter().map(|m| m.role).collect();
        assert_eq!(roles, ["agent_draft", "sent", "seller", "guard_block", "checkpoint"]);
        assert_eq!(db::negotiation(&db, id).unwrap().status, "awaiting_human");
        assert!(decide(&db, id, true, Some(990.0), 0).is_err(), "editing can only lower the price");
        decide(&db, id, true, None, 100 + 86_500).unwrap();
        let chat = scripted(vec![("propose_message", json!({"text": "dale, cerramos en 950", "intent": "conditional_accept", "price_pen": 950}))]);
        let t = advance(&db, id, "approval", 100 + 86_600, None, &chat, &|| false).unwrap();
        assert!(t.blocks.is_empty(), "{:?}", t.blocks);
        assert_eq!(db::negotiation(&db, id).unwrap().status, "deal_pending_payment");
        drop(db);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn low_trust_waits_for_a_checkpoint_and_bad_limits_are_refused() {
        let db = db::open(":memory:").unwrap();
        let mut c = case();
        c["trust"] = json!({"score": 30, "flags": ["pocas_resenas"]});
        let id = start(&db, None, &c, 0).unwrap();
        assert_eq!(pending_checkpoint(&db, id).unwrap().unwrap().meta["kind"], "continue_low_trust");
        c["target_pen"] = json!(1000.0);
        assert!(start(&db, None, &c, 0).is_err(), "target above walk-away");
    }
}
