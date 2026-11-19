mod agent;
mod db;
mod llm;
mod mcp;
mod stores;

use agent::{Expert, Store};
use serde_json::{Value, json};
use std::collections::HashSet;

const PLANNER: &str = include_str!("../prompts/planner.md");
const VERIFIER: &str = include_str!("../prompts/verifier.md");
const RANKER: &str = include_str!("../prompts/ranker.md");
const VERIFY_BATCH: usize = 15;
const MAX_ROUNDS: usize = 2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<(), Box<dyn std::error::Error>> = match args.first().map(String::as_str) {
        Some("mcp") => mcp::serve(args.get(1).map(String::as_str).unwrap_or("")).map_err(Into::into),
        Some("experts") => agent::load_experts("experts").map_err(Into::into).map(|experts| {
            for e in experts {
                println!("{:<12} mcp: {:<12} {}", e.name, e.mcp.join(","), e.description);
            }
        }),
        Some("skills") => agent::load_skills("skills").map_err(Into::into).map(|skills| {
            for s in skills {
                println!("{:<22} {}", s.name, s.description);
            }
        }),
        Some(_) => run(&args.join(" ")),
        None => {
            eprintln!(
                "uso:\n  obsession \"audífonos over-ear hasta 350 soles, Cusco\"\n  obsession experts | skills\n  obsession mcp <tienda>"
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn model(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.into())
}

/// The experts whose names the model chose; all of them if it chose none, so a round is never empty.
fn pick<'a>(experts: &'a [Expert], names: &Value) -> Vec<&'a Expert> {
    let chosen: Vec<&Expert> = experts
        .iter()
        .filter(|e| names.as_array().is_some_and(|n| n.iter().any(|x| x == e.name.as_str())))
        .collect();
    if chosen.is_empty() { experts.iter().collect() } else { chosen }
}

fn run(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let main_model = model("OBSESSION_MAIN", "commandcode-plan/deepseek/deepseek-v4.1-flash");
    // Different model family on purpose: the verifier should not share the finders' blind spots.
    let verifier_model = model("OBSESSION_VERIFIER", "zcode/GLM-5.3-Flash");
    let experts = agent::load_experts("experts")?;
    let skills = agent::load_skills("skills")?;
    let registry: Value =
        serde_json::from_str(&std::fs::read_to_string("mcp.json").map_err(|e| format!("mcp.json: {e}"))?)?;
    // The orchestrator only ever sees names and descriptions, never an expert's tools.
    let menu: Vec<Value> = experts.iter().map(|e| json!({"name": e.name, "description": e.description})).collect();

    println!("[plan] {main_model} estudia el producto…");
    let dossier = llm::chat_json(&main_model, PLANNER, &json!({"query": query, "experts": menu}).to_string())?;
    println!("[plan] necesidad: {}", dossier["need"].as_str().unwrap_or("?"));
    let mut chosen = pick(&experts, &dossier["experts"]);

    let store = Store::default();
    let mut verdicts: Vec<Value> = Vec::new();
    let mut verified_ids: HashSet<u64> = HashSet::new();
    let mut hint = String::new();
    let mut ranking = json!({});
    for round in 1..=MAX_ROUNDS {
        let names: Vec<&str> = chosen.iter().map(|e| e.name.as_str()).collect();
        println!("\n[ronda {round}] expertos: {}", names.join(", "));
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
                    s.spawn(move || (e.name.clone(), agent::run_expert(e, task, registry, skills, store)))
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        // Only what an expert chose to defend reaches the verifier.
        let mut items = Vec::new();
        for (name, sub) in &submissions {
            for p in sub["picks"].as_array().into_iter().flatten() {
                let Some(id) = p["id"].as_u64() else { continue };
                let Some(l) = store.get(id as usize) else { continue };
                if verified_ids.insert(id) {
                    let mut v = serde_json::to_value(&l)?;
                    v["id"] = json!(id);
                    v["expert"] = json!(name);
                    v["expert_why"] = p["why"].clone();
                    items.push(v);
                }
            }
        }
        if !items.is_empty() {
            println!("[verify] {verifier_model} revisa {} picks nuevos…", items.len());
            let sent: HashSet<u64> = items.iter().filter_map(|v| v["id"].as_u64()).collect();
            // A verdict for an id we did not send would "verify" a listing nobody checked.
            verdicts.extend(
                verify(&verifier_model, &main_model, &dossier, &items)
                    .into_iter()
                    .filter(|v| v["id"].as_u64().is_some_and(|id| sent.contains(&id))),
            );
        }
        let verified: Vec<Value> = verdicts
            .iter()
            .filter(|v| v["relevant"].as_bool() == Some(true))
            .filter_map(|v| {
                let l = store.get(v["id"].as_u64()? as usize)?;
                Some(json!({"id": v["id"], "store": l.store, "title": l.title, "price_pen": l.price_pen,
                    "seller": l.seller, "rating": l.rating, "sold": l.sold, "trust": v["trust"],
                    "true_cost_pen": v["true_cost_pen"], "flags": v["flags"], "note": v["note"]}))
            })
            .collect();
        println!("[verify] {} relevantes en total", verified.len());

        let reports: Vec<Value> = submissions
            .iter()
            .map(|(n, s)| json!({"expert": n, "tried": s["tried"], "missing": s["missing"]}))
            .collect();
        println!("[rank] {main_model} decide…");
        let input = json!({"query": query, "dossier": dossier, "verified_listings": verified,
            "expert_reports": reports, "experts": menu, "round": round, "max_rounds": MAX_ROUNDS});
        match llm::chat_json(&main_model, RANKER, &input.to_string()) {
            Ok(r) => ranking = r,
            // Keep the previous round's decision rather than lose the whole run.
            Err(e) if round > 1 => {
                println!("[rank] ronda {round} falló, me quedo con la anterior: {e}");
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
        println!("[rank] no satisfecho: {hint}");
    }

    let listings = store.all();
    let db = db::open("obsession.db")?;
    let run = db::new_run(&db, query, &dossier)?;
    db::save_listings(&db, run, &listings)?;
    for v in &verdicts {
        db::save_verdict(&db, run, v)?;
    }
    db::save_ranking(&db, run, &ranking)?;
    let csv_path = format!("run-{run}.csv");
    let rows = db::export_csv(&db, run, &csv_path)?;
    println!("\n== Veredicto ==\n{}\n", ranking["verdict"].as_str().unwrap_or("(sin veredicto)"));
    for (i, p) in ranking["picks"].as_array().into_iter().flatten().enumerate() {
        let Some(l) = p["id"].as_u64().and_then(|id| listings.get(id as usize)) else { continue };
        println!("{}. [{}] {} — S/ {:.2}\n   {}\n   {}", i + 1, l.store, l.title, l.price_pen, l.url,
            p["why"].as_str().unwrap_or(""));
    }
    println!("\n{} listings vistos, {rows} verificados en {csv_path} (run {run} en obsession.db)", listings.len());
    Ok(())
}

/// Batches go out in parallel; a batch that fails twice goes to the fallback model, then stays unverified.
fn verify(model: &str, fallback: &str, dossier: &Value, items: &[Value]) -> Vec<Value> {
    let batches: Vec<Value> =
        items.chunks(VERIFY_BATCH).map(|chunk| json!({"dossier": dossier, "listings": chunk})).collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = batches
            .iter()
            .map(|input| {
                s.spawn(move || {
                    llm::chat_json(model, VERIFIER, &input.to_string()).or_else(|e| {
                        // Losing the independent model beats leaving the batch unverified.
                        println!("[verify] {e}; reintento con {fallback}");
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
                    println!("[verify] lote falló: {e}");
                    None
                }
                Err(_) => None,
            })
            .flatten()
            .collect()
    })
}
