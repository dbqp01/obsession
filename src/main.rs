mod db;
mod llm;
mod stores;

use serde_json::{Value, json};
use std::collections::HashSet;
use stores::Listing;

const PLANNER: &str = include_str!("../prompts/planner.md");
const VERIFIER: &str = include_str!("../prompts/verifier.md");
const RANKER: &str = include_str!("../prompts/ranker.md");
const PER_STORE: usize = 12;
const VERIFY_BATCH: usize = 15;

fn main() {
    let query = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if query.trim().is_empty() {
        eprintln!("uso: obsession \"audífonos over-ear hasta 350 soles, Cusco\"");
        std::process::exit(2);
    }
    if let Err(e) = run(&query) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn model(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.into())
}

fn run(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let main_model = model("OBSESSION_MAIN", "commandcode-plan/deepseek/deepseek-v4.1-flash");
    // Different model family on purpose: the verifier should not share the finder's blind spots.
    let verifier_model = model("OBSESSION_VERIFIER", "zcode/GLM-5.3-Flash");

    println!("[plan] {main_model} estudia el producto…");
    let dossier = llm::chat_json(&main_model, PLANNER, query)?;
    let keywords: Vec<String> = dossier["keywords"]
        .as_array()
        .ok_or("el dossier no trae keywords")?
        .iter()
        .filter_map(|k| k.as_str().map(String::from))
        .take(5)
        .collect();
    println!("[plan] necesidad: {}", dossier["need"].as_str().unwrap_or("?"));
    println!("[plan] keywords: {}", keywords.join(" | "));

    let listings = search_all(&keywords);
    println!("[search] {} listings únicos", listings.len());
    if listings.is_empty() {
        return Err("ninguna tienda devolvió resultados".into());
    }

    let db = db::open("obsession.db")?;
    let run = db::new_run(&db, query, &dossier)?;
    db::save_listings(&db, run, &listings)?;

    println!("[verify] {verifier_model} revisa {} listings…", listings.len());
    let verdicts = verify(&verifier_model, &dossier, &listings);
    for v in &verdicts {
        db::save_verdict(&db, run, v)?;
    }
    let verified: Vec<Value> = verdicts
        .iter()
        .filter(|v| v["relevant"].as_bool() == Some(true))
        .filter_map(|v| {
            let l = listings.get(v["id"].as_u64()? as usize)?;
            Some(json!({"id": v["id"], "store": l.store, "title": l.title, "price_pen": l.price_pen,
                "seller": l.seller, "rating": l.rating, "sold": l.sold,
                "trust": v["trust"], "true_cost_pen": v["true_cost_pen"], "flags": v["flags"], "note": v["note"]}))
        })
        .collect();
    println!("[verify] {} de {} son relevantes", verified.len(), verdicts.len());

    println!("[rank] {main_model} decide…");
    let input = json!({"query": query, "dossier": dossier, "verified_listings": verified});
    let ranking = llm::chat_json(&main_model, RANKER, &input.to_string())?;
    db::save_ranking(&db, run, &ranking)?;

    let csv_path = format!("run-{run}.csv");
    let rows = db::export_csv(&db, run, &csv_path)?;
    println!("\n== Veredicto ==\n{}\n", ranking["verdict"].as_str().unwrap_or("(sin veredicto)"));
    for (i, pick) in ranking["picks"].as_array().into_iter().flatten().enumerate() {
        let Some(l) = pick["id"].as_u64().and_then(|id| listings.get(id as usize)) else { continue };
        println!("{}. [{}] {} — S/ {:.2}\n   {}\n   {}", i + 1, l.store, l.title, l.price_pen, l.url,
            pick["why"].as_str().unwrap_or(""));
    }
    println!("\n{rows} listings verificados en {csv_path} (run {run} en obsession.db)");
    Ok(())
}

/// One expert thread per store; each runs every keyword against its own store.
fn search_all(keywords: &[String]) -> Vec<Listing> {
    let client = stores::client();
    let per_store: Vec<Vec<Listing>> = std::thread::scope(|s| {
        let handles: Vec<_> = stores::STORES
            .iter()
            .map(|&store| {
                let client = &client;
                s.spawn(move || {
                    let mut found = Vec::new();
                    for kw in keywords {
                        match stores::search(client, store, kw) {
                            Ok(mut l) => {
                                println!("[search] {store} «{kw}»: {}", l.len());
                                found.append(&mut l);
                            }
                            Err(e) => {
                                println!("[search] {e}");
                                break; // a blocked store stays blocked for the other keywords
                            }
                        }
                    }
                    found
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_default()).collect()
    });
    let mut seen = HashSet::new();
    per_store
        .into_iter()
        .flat_map(|l| l.into_iter().filter(|x| seen.insert(x.url.clone())).take(PER_STORE).collect::<Vec<_>>())
        .collect()
}

/// Batches go out in parallel; a failed batch is reported and its listings stay unverified.
fn verify(model: &str, dossier: &Value, listings: &[Listing]) -> Vec<Value> {
    let batches: Vec<Value> = listings
        .chunks(VERIFY_BATCH)
        .enumerate()
        .map(|(b, chunk)| {
            let items: Vec<Value> = chunk
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let mut v = serde_json::to_value(l).unwrap();
                    v["id"] = json!(b * VERIFY_BATCH + i);
                    v
                })
                .collect();
            json!({"dossier": dossier, "listings": items})
        })
        .collect();
    std::thread::scope(|s| {
        let handles: Vec<_> = batches
            .iter()
            .map(|input| s.spawn(move || llm::chat_json(model, VERIFIER, &input.to_string())))
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
