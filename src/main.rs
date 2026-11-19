use obsession::{agent, eval, mcp};
use serde_json::Value;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(".");
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
        Some("eval") => eval::run(root, args.get(1).map(String::as_str), &print_event)
            .map(|out| println!("\n{}", serde_json::to_string_pretty(&out["summary"]).unwrap())),
        Some(_) => obsession::run(root, &args.join(" "), &print_event).map(|_| ()),
        None => {
            eprintln!(
                "uso:\n  obsession \"audífonos over-ear hasta 350 soles, Cusco\"\n  obsession eval [caso]\n  obsession experts | skills\n  obsession mcp <tienda>"
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn print_event(e: Value) {
    // OBSESSION_RECORD=ui/demo-events.jsonl saves a run the UI can replay in a plain browser.
    if let Some(path) = std::env::var_os("OBSESSION_RECORD") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{e}");
        }
    }
    match s(&e["kind"]) {
        "phase" => println!("\n» {}", s(&e["text"])),
        "plan" => println!("  necesidad: {}", s(&e["dossier"]["need"])),
        "round" => println!("\n[ronda {}] expertos: {}", e["round"], e["experts"]),
        "search" => println!("  [{}] busca «{}»: {}", s(&e["expert"]), s(&e["query"]), e["results"]),
        "expert_note" => println!("  [{}] {}", s(&e["expert"]), s(&e["text"])),
        "expert_done" => println!("  [{}] entregó {} picks en {} pasos", s(&e["expert"]), e["picks"], e["steps"]),
        "check" => println!("  {} [{}] {}: {}", if e["ok"] == true { "✓" } else { "✗" }, s(&e["store"]),
            s(&e["title"]).chars().take(60).collect::<String>(), s(&e["note"])),
        "not_satisfied" => println!("\n  no satisfecho: {}", s(&e["hint"])),
        "warning" => println!("  ! {}", s(&e["text"])),
        "eval_case" => println!("\n=== eval {}: {}", s(&e["id"]), s(&e["query"])),
        "done" => {
            let r = &e["report"];
            println!("\n== Veredicto ==\n{}\n", s(&r["verdict"]));
            for p in r["picks"].as_array().into_iter().flatten() {
                println!("{}. [{}] {} — S/ {}\n   {}\n   {}", p["rank"], s(&p["store"]), s(&p["title"]), p["price_pen"],
                    s(&p["url"]), s(&p["why"]));
            }
            println!("\n{} vistos, {} verificados, {} s, {}", r["seen"], r["verified"], r["elapsed_s"], s(&r["csv"]));
        }
        _ => {}
    }
}
