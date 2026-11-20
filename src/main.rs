use obsession::{connections, eval, mcp, packages};
use serde_json::Value;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(".");
    let result: Result<(), Box<dyn std::error::Error>> = match args.first().map(String::as_str) {
        Some("mcp") => mcp::serve(args.get(1).map(String::as_str).unwrap_or("")).map_err(Into::into),
        Some("experts") => {
            let l = loaded(root);
            let mut experts: Vec<_> = l.packages.iter().flat_map(|p| &p.experts).collect();
            experts.sort_by(|a, b| a.name.cmp(&b.name));
            for e in experts {
                println!("{:<12} mcp: {:<12} {}", e.name, e.mcp.join(","), e.description);
            }
            Ok(())
        }
        Some("skills") => {
            for s in loaded(root).packages.iter().flat_map(|p| &p.skills) {
                println!("{:<22} {}", s.name, s.description);
            }
            Ok(())
        }
        Some("packages") => {
            let l = loaded(root);
            for p in &l.packages {
                let m = &p.manifest;
                let experts: Vec<&str> = p.experts.iter().map(|e| e.name.as_str()).collect();
                let place = if p.builtin { "" } else { " (usuario)" };
                println!("{:<14} {:<8} {:<8} expertos: {}{place}", m.name, m.version, m.source, experts.join(","));
                for todo in packages::unmet(m, &l.registry) {
                    println!("  ! {todo}");
                }
            }
            Ok(())
        }
        Some("install") => user_dir().and_then(|user| {
            let (name, todo) = packages::install(Path::new(arg1(&args)), &root.join("packages"), &user)?;
            println!("Instalado {name} en {}", user.join(&name).display());
            for t in todo {
                println!("  ! {t}");
            }
            Ok(())
        }),
        Some("uninstall") => user_dir()
            .and_then(|user| packages::uninstall(arg1(&args), &root.join("packages"), &user).map_err(Into::into))
            .map(|_| println!("Desinstalado.")),
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
        Some("eval") => eval::run(root, args.get(1).map(String::as_str), &print_event)
            .map(|out| println!("\n{}", serde_json::to_string_pretty(&out["summary"]).unwrap())),
        Some(_) => obsession::run(root, &args.join(" "), &print_event).map(|_| ()),
        None => {
            eprintln!(
                "uso:\n  obsession \"audífonos over-ear hasta 350 soles, Cusco\"\n  obsession eval [caso]\n  obsession experts | skills | connections\n  obsession login <plataforma> | logout <plataforma>\n  obsession mcp <tienda>"
            );
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Loaded packages; refused ones are reported on stderr so listings stay clean.
fn loaded(root: &Path) -> packages::Loaded {
    let l = packages::load(root);
    for w in &l.warnings {
        eprintln!("! {w}");
    }
    l
}

fn user_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    packages::user_dir().ok_or_else(|| "no se encontró %LOCALAPPDATA%".into())
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
            println!("\n{} vistos, {} verificados, {} s, {}", r["seen"], r["verified"], r["elapsed_s"], s(&r["csv"]));
        }
        _ => {}
    }
}
