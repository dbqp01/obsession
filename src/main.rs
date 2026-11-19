mod db;
mod llm;
mod mcp;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("mcp") => mcp::serve(args.get(1).map(String::as_str).unwrap_or("")).map_err(Into::into),
        Some("experts") => load_catalog().map(|c| {
            for e in c {
                println!("{:<12} {:<14} {}", e["id"].as_str().unwrap_or("?"), e["region"].as_str().unwrap_or(""),
                    e["description"].as_str().unwrap_or(""));
            }
        }),
        Some(_) => run(&args.join(" ")),
        None => {
            eprintln!("uso:
  obsession \"audífonos over-ear hasta 350 soles, Cusco\"
  obsession experts
  obsession mcp <tienda>");
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

fn run(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let main_model = model("OBSESSION_MAIN", "commandcode-plan/deepseek/deepseek-v4.1-flash");
    // Different model family on purpose: the verifier should not share the finder's blind spots.
    let verifier_model = model("OBSESSION_VERIFIER", "zcode/GLM-5.3-Flash");

    let catalog = load_catalog()?;
    let menu: Vec<Value> = catalog
        .iter()
        .map(|e| json!({"id": e["id"], "description": e["description"], "region": e["region"], "categories": e["categories"]}))
        .collect();
    println!("[plan] {main_model} estudia el producto…");
    let dossier = llm::chat_json(&main_model, PLANNER, &json!({"query": query, "experts": menu}).to_string())?;
    let chosen: Vec<&Value> = catalog
        .iter()
        .filter(|e| dossier["experts"].as_array().is_some_and(|ids| ids.contains(&e["id"])))
        .collect();
    // A planner that picks nothing valid should not leave the swarm empty.
    let chosen = if chosen.is_empty() { catalog.iter().collect() } else { chosen };
    let keywords: Vec<String> = dossier["keywords"]
        .as_array()
        .ok_or("el dossier no trae keywords")?
        .iter()
        .filter_map(|k| k.as_str().map(String::from))
        .take(5)
        .collect();
    println!("[plan] necesidad: {}", dossier["need"].as_str().unwrap_or("?"));
    println!("[plan] keywords: {}", keywords.join(" | "));
    println!("[plan] expertos: {}", chosen.iter().filter_map(|e| e["id"].as_str()).collect::<Vec<_>>().join(", "));

    let listings = search_all(&chosen, &keywords);
    println!("[search] {} listings únicos", listings.len());
    if listings.is_empty() {
        return Err("ninguna tienda devolvió resultados".into());
    }

    let db = db::open("obsession.db")?;
    let run = db::new_run(&db, query, &dossier)?;
    db::save_listings(&db, run, &listings)?;

    println!("[verify] {verifier_model} revisa {} listings…", listings.len());
    let verdicts = verify(&verifier_model, &main_model, &dossier, &listings);
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

fn load_catalog() -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let path = std::env::var("OBSESSION_EXPERTS").unwrap_or_else(|_| "experts.json".into());
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let catalog: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    Ok(catalog["experts"].as_array().ok_or(format!("{path}: falta \"experts\""))?.clone())
}

/// The swarm: one MCP server per chosen expert, each in its own thread, running every keyword.
/// shortcut: an external expert must return `Listing`-shaped JSON; add a mapping step when one doesn't.
fn search_all(experts: &[&Value], keywords: &[String]) -> Vec<Listing> {
    let self_exe = std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let per_expert: Vec<Vec<Listing>> = std::thread::scope(|s| {
        let handles: Vec<_> = experts
            .iter()
            .map(|e| {
                let self_exe = &self_exe;
                s.spawn(move || {
                    let id = e["id"].as_str().unwrap_or("?");
                    let command = match e["command"].as_str() {
                        Some("self") | None => self_exe.as_str(),
                        Some(c) => c,
                    };
                    let args: Vec<String> =
                        e["args"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(String::from)).collect();
                    let tool = e["tool"].as_str().unwrap_or("search");
                    let mut client = match mcp::Client::spawn(command, &args) {
                        Ok(c) => c,
                        Err(err) => {
                            println!("[search] {id}: no arrancó ({err})");
                            return vec![];
                        }
                    };
                    let mut found = Vec::new();
                    for kw in keywords {
                        let result = client
                            .call(tool, json!({"query": kw}))
                            .and_then(|t| serde_json::from_str::<Vec<Listing>>(&t).map_err(|e| e.to_string()));
                        match result {
                            Ok(mut l) => {
                                println!("[search] {id} «{kw}»: {}", l.len());
                                found.append(&mut l);
                            }
                            Err(err) => {
                                println!("[search] {id}: {err}");
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
    per_expert
        .into_iter()
        .flat_map(|l| l.into_iter().filter(|x| seen.insert(x.url.clone())).take(PER_STORE).collect::<Vec<_>>())
        .collect()
}

/// Batches go out in parallel; a batch that fails twice goes to the fallback model, then stays unverified.
fn verify(model: &str, fallback: &str, dossier: &Value, listings: &[Listing]) -> Vec<Value> {
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
