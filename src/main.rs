use obsession::{agent, connections, eval, mcp, providers};
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
        Some("connections") => obsession::load_registry(root).map(|reg| {
            for c in connections::list(&reg) {
                let state = if c["connected"] == true { "conectado" } else { "sin sesión" };
                println!("{:<12} {:<12} {}", s(&c["id"]), state, s(&c["site"]));
            }
        }),
        Some("login") => obsession::load_registry(root)
            .and_then(|reg| connections::start_login(&reg, arg1(&args)).map_err(Into::into))
            .map(|_| println!("Se abrió el login: inicia sesión en el navegador y sigue las instrucciones de esa consola.")),
        Some("logout") => obsession::load_registry(root)
            .and_then(|reg| connections::disconnect(&reg, arg1(&args)).map_err(Into::into))
            .map(|_| println!("Sesión borrada.")),
        Some("provider") => match (args.get(1).map(String::as_str), args.get(2), args.get(3)) {
            (Some("add"), Some(name), Some(url)) => {
                providers::add(name, url).map_err(Into::into).map(|_| println!("Proveedor «{name}» guardado."))
            }
            (Some("list"), _, _) => providers::list().map_err(Into::into).map(|list| {
                for (name, url, has_key) in list {
                    println!("{name:<12} {url:<40} {}", if has_key { "clave guardada" } else { "sin clave" });
                }
            }),
            _ => Err(USAGE.into()),
        },
        Some("key") => match (args.get(1).map(String::as_str), args.get(2)) {
            (Some("set"), Some(name)) => read_key()
                .and_then(|key| providers::set_key(name, &key).map_err(Into::into))
                .map(|_| println!("Clave de «{name}» guardada cifrada (DPAPI).")),
            (Some("remove"), Some(name)) => providers::remove_key(name)
                .map_err(Into::into)
                .map(|had| println!("{}", if had { "Clave borrada." } else { "No había clave guardada." })),
            _ => Err(USAGE.into()),
        },
        Some("eval") => eval::run(root, args.get(1).map(String::as_str), &print_event)
            .map(|out| println!("\n{}", serde_json::to_string_pretty(&out["summary"]).unwrap())),
        Some(_) => obsession::run(root, &args.join(" "), &print_event).map(|_| ()),
        None => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

const USAGE: &str = "uso:
  obsession \"audífonos over-ear hasta 350 soles, Cusco\"
  obsession eval [caso]
  obsession experts | skills | connections
  obsession login <plataforma> | logout <plataforma>
  obsession provider add <nombre> <base_url> | provider list
  obsession key set <proveedor>     (lee la clave de stdin, nunca de los argumentos)
  obsession key remove <proveedor>
  obsession mcp <tienda>
modelos: byok:<proveedor>/<modelo> usa tu proveedor; cualquier otro id va por magpie";

/// The key comes from stdin so it never lands in shell history or the process list.
fn read_key() -> Result<String, Box<dyn std::error::Error>> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        eprintln!("Pega la clave y pulsa Enter:");
    }
    let mut key = String::new();
    std::io::stdin().read_line(&mut key)?;
    Ok(key.trim().to_string())
}

fn arg1(args: &[String]) -> &str {
    args.get(1).map(String::as_str).unwrap_or("")
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
            let t = &r["tokens"];
            println!("\n{} vistos, {} verificados, {} s, {} tokens ({} entrada + {} salida), {}", r["seen"],
                r["verified"], r["elapsed_s"], t["total"], t["prompt"], t["completion"], s(&r["csv"]));
        }
        _ => {}
    }
}
