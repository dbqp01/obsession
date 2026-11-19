//! Obsession's core: plan → expert agents → page checks → verifier → ranker, in rounds.
//! The CLI and the desktop app both call `run` and render the events it emits.
pub mod agent;
pub mod db;
pub mod eval;
pub mod llm;
pub mod mcp;
pub mod stores;

use agent::{Expert, Store};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

/// Progress sink: every step of a run is reported as one JSON event with a `kind`.
pub type Emit<'a> = &'a (dyn Fn(Value) + Sync);

const PLANNER: &str = include_str!("../prompts/planner.md");
const VERIFIER: &str = include_str!("../prompts/verifier.md");
const RANKER: &str = include_str!("../prompts/ranker.md");
const VERIFY_BATCH: usize = 15;
const MAX_ROUNDS: usize = 2;
/// A page price this far from the listing price means the listing was stale.
const PRICE_TOLERANCE: f64 = 0.05;

pub fn model(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.into())
}

pub fn main_model() -> String {
    model("OBSESSION_MAIN", "commandcode-plan/deepseek/deepseek-v4.1-flash")
}

/// The experts whose names the model chose; all of them if it chose none, so a round is never empty.
fn pick<'a>(experts: &'a [Expert], names: &Value) -> Vec<&'a Expert> {
    let chosen: Vec<&Expert> = experts
        .iter()
        .filter(|e| names.as_array().is_some_and(|n| n.iter().any(|x| x == e.name.as_str())))
        .collect();
    if chosen.is_empty() { experts.iter().collect() } else { chosen }
}

/// Runs one purchase search end to end and returns the report the UI shows.
/// `root` holds `experts/`, `skills/`, `mcp.json` and receives `obsession.db` and `runs/`.
pub fn run(root: &Path, query: &str, emit: Emit) -> Result<Value, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let main_model = main_model();
    // A different model family on purpose: the verifier should not share the finders' blind spots.
    let verifier_model = model("OBSESSION_VERIFIER", "commandcode-plan/z-ai/glm-5.3-flash");
    let experts = agent::load_experts(&root.join("experts").to_string_lossy())?;
    let skills = agent::load_skills(&root.join("skills").to_string_lossy())?;
    let registry_path = root.join("mcp.json");
    let registry: Value = serde_json::from_str(
        &std::fs::read_to_string(&registry_path).map_err(|e| format!("{}: {e}", registry_path.display()))?,
    )?;
    // The orchestrator only ever sees names and descriptions, never an expert's tools.
    let menu: Vec<Value> = experts.iter().map(|e| json!({"name": e.name, "description": e.description})).collect();

    emit(json!({"kind": "phase", "phase": "plan", "text": "Estudiando el producto y la necesidad"}));
    let dossier = llm::chat_json(&main_model, PLANNER, &json!({"query": query, "experts": menu}).to_string())?;
    emit(json!({"kind": "plan", "dossier": dossier}));
    let mut chosen = pick(&experts, &dossier["experts"]);

    let store = Store::default();
    let mut checks: HashMap<u64, stores::PageCheck> = HashMap::new();
    let mut rejected: Vec<Value> = Vec::new();
    let mut verdicts: Vec<Value> = Vec::new();
    let mut sent_ids: HashSet<u64> = HashSet::new();
    let mut hint = String::new();
    let mut ranking = json!({});
    let mut rounds = 0;
    for round in 1..=MAX_ROUNDS {
        rounds = round;
        let names: Vec<&str> = chosen.iter().map(|e| e.name.as_str()).collect();
        emit(json!({"kind": "round", "round": round, "experts": names, "hint": hint}));
        let task = json!({
            "query": query, "need": dossier["need"], "budget_pen": dossier["budget_pen"],
            "location": dossier["location"], "fair_price_pen": dossier["fair_price_pen"],
            "candidates": dossier["candidates"], "category_jumps": dossier["category_jumps"],
            "defects_to_watch": dossier["defects_to_watch"], "orchestrator_hint": hint,
        })
        .to_string();
        let submissions: Vec<(String, Value)> = std::thread::scope(|s| {
            let handles: Vec<_> = chosen
                .iter()
                .map(|e| {
                    let (task, registry, skills, store) = (&task, &registry, &skills, &store);
                    s.spawn(move || {
                        let sub = agent::run_expert(e, task, registry, skills, store, emit);
                        emit(json!({"kind": "expert_done", "expert": e.name, "steps": sub["steps"],
                            "picks": sub["picks"].as_array().map_or(0, |p| p.len()), "missing": sub["missing"]}));
                        (e.name.clone(), sub)
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        // Only what an expert chose to defend goes on; each of those pages is opened again.
        let mut new_ids = Vec::new();
        for (_, sub) in &submissions {
            for p in sub["picks"].as_array().into_iter().flatten() {
                if let Some(id) = p["id"].as_u64().filter(|id| store.get(*id as usize).is_some()) {
                    if sent_ids.insert(id) {
                        new_ids.push(id);
                    }
                }
            }
        }
        emit(json!({"kind": "phase", "phase": "check", "text": format!("Abriendo {} páginas de producto", new_ids.len())}));
        let opened = open_pages(&registry, &store, &new_ids);
        let mut items = Vec::new();
        for id in new_ids {
            let Some(mut l) = store.get(id as usize) else { continue };
            let check = opened.get(&id).cloned().unwrap_or_else(|| stores::PageCheck {
                url: l.url.clone(), ok: false, title: None, price_pen: None, in_stock: None, seller: None,
                rating: None, reviews: None, method: "page".into(), note: "no se pudo abrir".into(),
            });
            let reason = if !check.ok {
                Some(format!("la página no confirmó el producto: {}", check.note))
            } else if check.in_stock == Some(false) {
                Some("sin stock en la página".into())
            } else {
                None
            };
            emit(json!({"kind": "check", "id": id, "title": l.title, "store": l.store, "ok": reason.is_none(),
                "listing_price": l.price_pen, "page_price": check.price_pen, "note": reason.clone().unwrap_or(check.note.clone())}));
            if let Some(reason) = reason {
                rejected.push(json!({"id": id, "store": l.store, "title": l.title, "url": l.url, "reason": reason}));
                checks.insert(id, check);
                continue;
            }
            // The page is the source of truth for price and seller.
            if let Some(p) = check.price_pen {
                l.price_pen = p;
            }
            if check.seller.is_some() {
                l.seller = check.seller.clone();
            }
            let mut v = serde_json::to_value(&l)?;
            v["id"] = json!(id);
            v["page"] = serde_json::to_value(&check)?;
            items.push(v);
            checks.insert(id, check);
        }
        if !items.is_empty() {
            emit(json!({"kind": "phase", "phase": "verify", "text": format!("Verificando {} opciones con otro modelo", items.len())}));
            let sent: HashSet<u64> = items.iter().filter_map(|v| v["id"].as_u64()).collect();
            // A verdict for an id we did not send would "verify" a listing nobody checked.
            verdicts.extend(
                verify(&verifier_model, &main_model, &dossier, &items, emit)
                    .into_iter()
                    .filter(|v| v["id"].as_u64().is_some_and(|id| sent.contains(&id))),
            );
        }
        let verified = verified_listings(&store, &checks, &verdicts);
        let reports: Vec<Value> = submissions
            .iter()
            .map(|(n, s)| json!({"expert": n, "tried": s["tried"], "missing": s["missing"]}))
            .collect();
        emit(json!({"kind": "phase", "phase": "rank", "text": "Decidiendo la mejor compra"}));
        let input = json!({"query": query, "dossier": dossier, "verified_listings": verified, "rejected": rejected,
            "expert_reports": reports, "experts": menu, "round": round, "max_rounds": MAX_ROUNDS});
        match llm::chat_json(&main_model, RANKER, &input.to_string()) {
            Ok(r) => ranking = r,
            // Keep the previous round's decision rather than lose the whole run.
            Err(e) if round > 1 => {
                emit(json!({"kind": "warning", "text": format!("la ronda {round} no pudo decidir; se mantiene la anterior: {e}")}));
                break;
            }
            Err(e) => return Err(e.into()),
        }
        let relevant: HashSet<u64> = verified.iter().filter_map(|v| v["id"].as_u64()).collect();
        if let Some(picks) = ranking["picks"].as_array_mut() {
            picks.retain(|p| p["id"].as_u64().is_some_and(|id| relevant.contains(&id)));
        }
        if ranking["satisfied"].as_bool() != Some(false) || round == MAX_ROUNDS {
            break;
        }
        hint = ranking["next_round"]["hint"].as_str().unwrap_or("").to_string();
        chosen = pick(&experts, &ranking["next_round"]["experts"]);
        emit(json!({"kind": "not_satisfied", "hint": hint}));
    }

    let verified = verified_listings(&store, &checks, &verdicts);
    let by_id: HashMap<u64, &Value> = verified.iter().filter_map(|v| Some((v["id"].as_u64()?, v))).collect();
    let picks: Vec<Value> = ranking["picks"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, p)| {
            let mut v = (*by_id.get(&p["id"].as_u64()?)?).clone();
            v["rank"] = json!(i + 1);
            v["why"] = p["why"].clone();
            Some(v)
        })
        .collect();

    let listings = store.all();
    let db = db::open(&root.join("obsession.db").to_string_lossy())?;
    let run_id = db::new_run(&db, query, &dossier)?;
    db::save_listings(&db, run_id, &listings)?;
    for v in &verdicts {
        db::save_verdict(&db, run_id, v)?;
    }
    db::save_ranking(&db, run_id, &ranking)?;
    std::fs::create_dir_all(root.join("runs"))?;
    let csv_path = root.join("runs").join(format!("run-{run_id}.csv"));
    db::export_csv(&db, run_id, &csv_path.to_string_lossy())?;
    let report = json!({
        "run_id": run_id, "query": query, "dossier": dossier, "rounds": rounds,
        "verdict": ranking["verdict"], "satisfied": ranking["satisfied"], "picks": picks, "rejected": rejected,
        "seen": listings.len(), "verified": verified.len(), "elapsed_s": started.elapsed().as_secs(),
        "csv": csv_path.to_string_lossy(),
    });
    db::save_report(&db, run_id, &report)?;
    emit(json!({"kind": "done", "report": report}));
    Ok(report)
}

/// Relevant verdicts joined with their listing and page check, as the ranker and the UI see them.
fn verified_listings(store: &Store, checks: &HashMap<u64, stores::PageCheck>, verdicts: &[Value]) -> Vec<Value> {
    verdicts
        .iter()
        .filter(|v| v["relevant"].as_bool() == Some(true))
        .filter_map(|v| {
            let id = v["id"].as_u64()?;
            let l = store.get(id as usize)?;
            let page = checks.get(&id)?;
            Some(json!({"id": id, "store": l.store, "title": l.title, "url": l.url,
                "price_pen": page.price_pen.unwrap_or(l.price_pen), "listing_price_pen": l.price_pen,
                "seller": page.seller.clone().or(l.seller), "rating": page.rating.or(l.rating),
                "reviews": page.reviews, "sold": l.sold, "in_stock": page.in_stock, "check": page.method,
                "trust": v["trust"], "true_cost_pen": v["true_cost_pen"], "flags": v["flags"], "note": v["note"]}))
        })
        .collect()
}

/// Opens each listing through its own store's MCP `open` tool, one server per store, in parallel.
fn open_pages(registry: &Value, store: &Store, ids: &[u64]) -> HashMap<u64, stores::PageCheck> {
    let mut by_store: HashMap<String, Vec<(u64, stores::Listing)>> = HashMap::new();
    for &id in ids {
        if let Some(l) = store.get(id as usize) {
            by_store.entry(l.store.clone()).or_default().push((id, l));
        }
    }
    std::thread::scope(|s| {
        let handles: Vec<_> = by_store
            .into_iter()
            .map(|(name, items)| {
                s.spawn(move || {
                    let Some(cfg) = registry["servers"].get(&name) else { return vec![] };
                    let Ok(mut client) = mcp::Client::from_config(cfg) else { return vec![] };
                    items
                        .into_iter()
                        .filter_map(|(id, l)| {
                            let text = client.call("open", json!({"url": l.url, "title": l.title})).ok()?;
                            Some((id, serde_json::from_str::<stores::PageCheck>(&text).ok()?))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).flatten().collect()
    })
}

/// Batches go out in parallel; a batch that fails twice goes to the fallback model, then stays unverified.
fn verify(model: &str, fallback: &str, dossier: &Value, items: &[Value], emit: Emit) -> Vec<Value> {
    let batches: Vec<Value> =
        items.chunks(VERIFY_BATCH).map(|chunk| json!({"dossier": dossier, "listings": chunk})).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = batches
            .iter()
            .map(|input| {
                s.spawn(move || {
                    llm::chat_json(model, VERIFIER, &input.to_string()).or_else(|e| {
                        // Losing the independent model beats leaving the batch unverified.
                        emit(json!({"kind": "warning", "text": format!("{e}; reintento con {fallback}")}));
                        llm::chat_json(fallback, VERIFIER, &input.to_string())
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| match h.join() {
                Ok(Ok(v)) => v["verdicts"].as_array().cloned(),
                Ok(Err(e)) => {
                    emit(json!({"kind": "warning", "text": format!("un lote quedó sin verificar: {e}")}));
                    None
                }
                Err(_) => None,
            })
            .flatten()
            .collect()
    })
}

/// True when the page price is within tolerance of the listed price (both must exist).
pub fn price_matches(listing: f64, page: Option<f64>) -> bool {
    page.is_some_and(|p| listing > 0.0 && ((p - listing).abs() / listing) <= PRICE_TOLERANCE)
}

#[cfg(test)]
mod tests {
    use super::price_matches;

    #[test]
    fn price_tolerance() {
        assert!(price_matches(100.0, Some(104.9)));
        assert!(!price_matches(100.0, Some(106.0)));
        assert!(!price_matches(100.0, None));
        assert!(!price_matches(0.0, Some(0.0)));
    }
}
