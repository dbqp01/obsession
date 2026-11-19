//! `obsession eval`: runs the fixed cases, re-checks every pick against its page, asks a model
//! with no tools for its answer, and writes a side-by-side report for a human to judge.
use crate::{Emit, llm, stores};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Instant;

const BASELINE: &str = "Eres un asistente de compras sin acceso a internet. Responde de memoria. \
Recomienda la mejor compra para la consulta. Responde SOLO JSON: \
{\"pick\": \"marca y modelo\", \"price_pen\": número estimado, \"where\": \"dónde comprarlo\", \"why\": \"una frase\"}";
const TIME_LIMIT_S: u64 = 300;

/// A pick is false if its page no longer confirms it, its price moved more than the tolerance,
/// or the verifier itself flagged it as an accessory.
fn pick_problem(pick: &Value, recheck: &stores::PageCheck) -> Option<String> {
    if !recheck.ok {
        return Some(format!("no abre: {}", recheck.note));
    }
    let shown = pick["price_pen"].as_f64().unwrap_or(0.0);
    if !crate::price_matches(shown, recheck.price_pen) {
        return Some(format!("precio mostrado S/{shown:.2}, página S/{:.2}", recheck.price_pen.unwrap_or(0.0)));
    }
    if pick["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == "accesorio")) {
        return Some("es un accesorio".into());
    }
    None
}

pub fn run(root: &Path, only: Option<&str>, emit: Emit) -> Result<Value, Box<dyn std::error::Error>> {
    let cases: Value = serde_json::from_str(&std::fs::read_to_string(root.join("eval/cases.json"))?)?;
    let client = stores::client();
    let main_model = crate::main_model();
    let mut rows = Vec::new();
    for case in cases["cases"].as_array().ok_or("eval/cases.json sin cases")? {
        let id = case["id"].as_str().unwrap_or("?");
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let query = case["query"].as_str().unwrap_or("");
        emit(json!({"kind": "eval_case", "id": id, "query": query}));
        let started = Instant::now();
        let result = crate::run(root, query, emit);
        let elapsed = started.elapsed().as_secs();
        let baseline = llm::chat_json(&main_model, BASELINE, query).unwrap_or_else(|e| json!({"error": e}));
        let row = match result {
            Err(e) => json!({"id": id, "query": query, "ok": false, "error": e.to_string(), "elapsed_s": elapsed,
                "baseline": baseline}),
            Ok(report) => {
                let picks: Vec<Value> = report["picks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        let recheck = stores::open(&client, p["store"].as_str().unwrap_or(""),
                            p["url"].as_str().unwrap_or(""), p["title"].as_str().unwrap_or(""));
                        json!({"title": p["title"], "store": p["store"], "url": p["url"], "price_pen": p["price_pen"],
                            "problem": pick_problem(p, &recheck)})
                    })
                    .collect();
                json!({"id": id, "query": query, "ok": true, "elapsed_s": elapsed, "rounds": report["rounds"],
                    "verdict": report["verdict"], "picks": picks, "run_id": report["run_id"], "baseline": baseline})
            }
        };
        emit(json!({"kind": "eval_row", "row": row}));
        rows.push(row);
    }
    let summary = summarize(&rows);
    let out = json!({"summary": summary, "rows": rows});
    std::fs::create_dir_all(root.join("eval/results"))?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    std::fs::write(root.join(format!("eval/results/{stamp}.json")), serde_json::to_string_pretty(&out)?)?;
    std::fs::write(root.join(format!("eval/results/{stamp}.md")), markdown(&out))?;
    Ok(out)
}

fn summarize(rows: &[Value]) -> Value {
    let total = rows.len();
    let ok = rows.iter().filter(|r| r["ok"] == true).count();
    let fast = rows.iter().filter(|r| r["ok"] == true && r["elapsed_s"].as_u64().is_some_and(|s| s <= TIME_LIMIT_S)).count();
    let picks: Vec<&Value> = rows.iter().flat_map(|r| r["picks"].as_array().into_iter().flatten()).collect();
    let false_picks = picks.iter().filter(|p| !p["problem"].is_null()).count();
    let second_rounds = rows.iter().filter(|r| r["rounds"].as_u64().is_some_and(|n| n >= 2)).count();
    json!({"cases": total, "completed": ok, "under_5_min": fast, "picks": picks.len(),
        "false_picks": false_picks, "second_rounds": second_rounds})
}

fn markdown(out: &Value) -> String {
    let s = &out["summary"];
    let mut md = format!(
        "# Eval de Obsession\n\n| Criterio | Resultado | Meta |\n|---|---|---|\n\
         | Corridas sin error | {}/{} | 10/10 |\n| Menos de 5 min | {}/{} | 10/10 |\n\
         | Picks falsos | {} de {} | 0 |\n| Casos con ronda 2 | {} | ≥ 1 que mejore |\n\
         | Obsession ≥ IA sin tools | (lo marcas tú abajo) | ≥ 7/10 |\n\n",
        s["completed"], s["cases"], s["under_5_min"], s["cases"], s["false_picks"], s["picks"], s["second_rounds"]
    );
    for r in out["rows"].as_array().into_iter().flatten() {
        md += &format!("## {} — {}\n\n", r["id"].as_str().unwrap_or(""), r["query"].as_str().unwrap_or(""));
        if r["ok"] != true {
            md += &format!("**Error:** {}\n\n", r["error"].as_str().unwrap_or(""));
        } else {
            md += &format!("{} s · {} ronda(s)\n\n**Obsession:** {}\n\n", r["elapsed_s"], r["rounds"],
                r["verdict"].as_str().unwrap_or("(sin veredicto)"));
            for p in r["picks"].as_array().into_iter().flatten() {
                let status = p["problem"].as_str().map(|x| format!(" ⚠ {x}")).unwrap_or(" ✓".into());
                md += &format!("- [{}] {} — S/{} ({}){status}\n", p["store"].as_str().unwrap_or(""),
                    p["title"].as_str().unwrap_or(""), p["price_pen"], p["url"].as_str().unwrap_or(""));
            }
        }
        let b = &r["baseline"];
        md += &format!("\n**IA sin tools:** {} — S/{} en {}. {}\n\n**Juez (mejor / igual / peor):** \n\n",
            b["pick"].as_str().unwrap_or("?"), b["price_pen"], b["where"].as_str().unwrap_or("?"), b["why"].as_str().unwrap_or(""));
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(ok: bool, price: Option<f64>) -> stores::PageCheck {
        stores::PageCheck { url: "u".into(), ok, title: None, price_pen: price, in_stock: None, seller: None,
            rating: None, reviews: None, method: "page".into(), note: "x".into() }
    }

    #[test]
    fn flags_false_picks() {
        let pick = json!({"price_pen": 100.0, "flags": []});
        assert!(pick_problem(&pick, &check(true, Some(102.0))).is_none());
        assert!(pick_problem(&pick, &check(true, Some(120.0))).unwrap().contains("precio"));
        assert!(pick_problem(&pick, &check(false, None)).unwrap().starts_with("no abre"));
        let accessory = json!({"price_pen": 100.0, "flags": ["accesorio"]});
        assert_eq!(pick_problem(&accessory, &check(true, Some(100.0))).unwrap(), "es un accesorio");
    }
}
