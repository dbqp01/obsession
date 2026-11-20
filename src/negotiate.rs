//! The buyer-side negotiator: guards, approvals and message limits (code, not prompt), and the turn loop.
//! v0 is copilot only: the agent drafts, a human sends. No code path here talks to a real platform.
use regex::Regex;
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
}
