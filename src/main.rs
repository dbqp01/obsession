use obsession::negotiate::{self as neg, Outcome};
use obsession::{connections, db, eval, llm, mcp, negsim, packages, providers};
use serde_json::{Value, json};
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
        Some("search") => packages::search(&args[1..].join(" ")).map_err(Into::into).map(|found| {
            if found.is_empty() {
                println!("No hay paquetes que coincidan.");
            }
            for (name, version, description) in found {
                println!("{name:<36} {version:<8} {description}");
            }
        }),
        Some("install") => user_dir().and_then(|user| {
            let target = arg1(&args);
            // `@scope/name` comes from the marketplace on npm; anything else is a local folder.
            let (name, todo) = if target.starts_with('@') {
                packages::install_npm(target, &root.join("packages"), &user)?
            } else {
                packages::install(Path::new(target), &root.join("packages"), &user)?
            };
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
        Some("negotiate") => negotiate(root, &args[1..]),
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
  obsession experts | skills | packages | connections
  obsession search [texto]                          (marketplace en npm)
  obsession install <carpeta> | install @akimibks/obsession-<nombre>[@x.y.z]
  obsession uninstall <paquete>
  obsession login <plataforma> | logout <plataforma>
  obsession provider add <nombre> <base_url> | provider list
  obsession key set <proveedor>     (lee la clave de stdin, nunca de los argumentos)
  obsession key remove <proveedor>
  obsession mcp <tienda>
  obsession negotiate start <run_id> <rank> [--target N --walkaway N]
  obsession negotiate start --title T --ask N --target N --walkaway N [--url U] [--trust N] [--evidence PRECIO:título]...
  obsession negotiate show [id] | reply <id>   (reply lee la respuesta del vendedor de stdin)
  obsession negotiate approve <id> [--price N] | reject <id>
  obsession negotiate eval [caso|persona]   (simulador; escribe eval/results/negotiation-<fecha>.md)
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
            let t = &r["tokens"];
            println!("\n{} vistos, {} verificados, {} s, {} tokens ({} entrada + {} salida), {}", r["seen"],
                r["verified"], r["elapsed_s"], t["total"], t["prompt"], t["completion"], s(&r["csv"]));
        }
        _ => {}
    }
}

type Res = Result<(), Box<dyn std::error::Error>>;

/// Copilot negotiation: the agent drafts, you send from your own account and paste the reply back.
fn negotiate(root: &Path, args: &[String]) -> Res {
    if args.first().map(String::as_str) == Some("eval") {
        return negsim::run(root, args.get(1).map(String::as_str), &|line| println!("{line}"))
            .map(|out| println!("
{}", serde_json::to_string_pretty(&out["summary"]).unwrap()));
    }
    let db = db::open(&root.join("obsession.db").to_string_lossy())?;
    let now = neg::now();
    let usage = llm::Usage::from_env();
    let model = neg::negotiator_model();
    let chat = |m: &[Value], t: &[Value], force: bool| llm::chat_tools(&model, m, t, force, &usage);
    let spent = || usage.spent();
    let skill = neg::skill_body(root);
    let skill = skill.as_deref();
    let pos = |i: usize| args.get(i).filter(|a| !a.starts_with("--")).map(String::as_str);
    let num = |name: &str| -> Result<Option<f64>, String> {
        flag(args, name).map(|v| v.parse::<f64>().map_err(|_| format!("{name} no es un número: {v}"))).transpose()
    };
    let id = || -> Result<i64, String> { pos(1).and_then(|v| v.parse().ok()).ok_or_else(|| "falta el id de la negociación".into()) };
    match args.first().map(String::as_str) {
        Some("start") => {
            let (run_id, mut case) = match (pos(1), pos(2)) {
                (Some(run), Some(rank)) => {
                    let run: i64 = run.parse().map_err(|_| "run_id no válido")?;
                    (Some(run), neg::case_from_pick(&db, run, rank.parse().map_err(|_| "rank no válido")?)?)
                }
                _ => {
                    let evidence: Vec<Value> = flags(args, "--evidence")
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let (p, title) = e.split_once(':').ok_or(format!("--evidence va como PRECIO:título: {e}"))?;
                            let p: f64 = p.trim().parse().map_err(|_| format!("precio no válido en --evidence: {p}"))?;
                            Ok(json!({"id": format!("e{}", i + 1), "title": title.trim(), "price_pen": p}))
                        })
                        .collect::<Result<_, String>>()?;
                    let ask = num("--ask")?.ok_or("falta --ask (o usa <run_id> <rank>)")?;
                    (None, json!({"listing": {"title": flag(args, "--title").unwrap_or(""), "url": flag(args, "--url").unwrap_or(""),
                        "ask_pen": ask}, "evidence": evidence, "trust": {"score": num("--trust")?.map(|t| t as i64)},
                        "mode": "copilot"}))
                }
            };
            let (Some(target), Some(walkaway)) = (num("--target")?, num("--walkaway")?) else {
                println!("Propuesta para «{}» (pide S/ {}):\n  target S/ {}  walk-away S/ {}\n  {} comparables verificados",
                    s(&case["listing"]["title"]), case["listing"]["ask_pen"], case["target_pen"], case["walkaway_pen"],
                    case["evidence"].as_array().map_or(0, |e| e.len()));
                println!("Confírmalos (o edítalos) repitiendo el comando con --target N --walkaway N.");
                return Ok(());
            };
            case["target_pen"] = json!(target);
            case["walkaway_pen"] = json!(walkaway);
            let id = neg::start(&db, run_id, &case, now)?;
            println!("Negociación {id} creada en modo copiloto.");
            if neg::pending_checkpoint(&db, id)?.is_some() {
                println!("El vendedor tiene trust bajo: decide con `obsession negotiate approve {id}` o `reject {id}`.");
                return Ok(());
            }
            print_turn(id, &neg::advance(&db, id, "start", now, skill, &chat, &spent)?);
        }
        Some("show") => match pos(1) {
            None => {
                for id in db::negotiation_ids(&db)? {
                    let n = db::negotiation(&db, id)?;
                    println!("{id:<4} {:<22} nosotros S/ {:<8} vendedor S/ {:<8} {}", n.status, opt(n.our_offer),
                        opt(n.seller_offer), s(&n.case["listing"]["title"]));
                }
            }
            Some(_) => {
                let id = id()?;
                let n = db::negotiation(&db, id)?;
                let c = &n.case;
                println!("{} — pide S/ {} · target S/ {} · walk-away S/ {}\nestado: {} · nosotros S/ {} · vendedor S/ {} · concesiones {} · señales {:?}\n",
                    s(&c["listing"]["title"]), c["listing"]["ask_pen"], c["target_pen"], c["walkaway_pen"], n.status,
                    opt(n.our_offer), opt(n.seller_offer), n.concessions, n.signals);
                for m in db::neg_messages(&db, id)? {
                    println!("[{}] {}", m.role, m.text);
                }
                if let Some(hint) = neg::limit_hint(&db, id, now)? {
                    println!("\nSugerencia de ritmo: {hint}.");
                }
            }
        },
        Some("reply") => {
            let id = id()?;
            use std::io::{IsTerminal, Read};
            if std::io::stdin().is_terminal() {
                eprintln!("Pega la respuesta del vendedor y termina con Ctrl+Z y Enter:");
            }
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            print_turn(id, &neg::reply(&db, id, &text, now, skill, &chat, &spent)?);
        }
        Some(verb @ ("approve" | "reject")) => {
            let id = id()?;
            let approved = verb == "approve";
            let a = neg::decide(&db, id, approved, num("--price")?, now)?;
            println!("{} {} {}.", if approved { "Aprobado" } else { "Rechazado" }, a.kind,
                a.price_pen.map(|p| format!("en S/ {p}")).unwrap_or_default());
            let event = if approved { "approval" } else { "rejection" };
            print_turn(id, &neg::advance(&db, id, event, now, skill, &chat, &spent)?);
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn print_turn(id: i64, t: &neg::Turn) {
    for (b, draft) in &t.blocks {
        println!("  guarda {}: {} («{draft}»)", b.guard, b.reason);
    }
    match &t.outcome {
        Outcome::Message { text, .. } => {
            println!("\nBorrador (cópialo y envíalo tú desde tu cuenta):\n  {text}");
            println!("Cuando responda: obsession negotiate reply {id}");
        }
        Outcome::Checkpoint { kind, price, terms, summary, warnings } => {
            println!("\nCheckpoint {kind}: {summary}\n  precio: {}  condiciones: {terms}",
                price.map(|p| format!("S/ {p}")).unwrap_or("-".into()));
            for w in warnings {
                println!("  ! {w}");
            }
            println!("Decide: obsession negotiate approve {id} [--price N] | reject {id}");
        }
        Outcome::WalkAway { reason, final_message } => {
            println!("\nEl agente propone retirarse: {reason}");
            if let Some(m) = final_message {
                println!("Mensaje de despedida (envíalo tú):\n  {m}");
            }
        }
        Outcome::Nothing(note) => println!("\nSin mensaje: {note}"),
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn flags<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.windows(2).filter(|w| w[0] == name).map(|w| w[1].as_str()).collect()
}

fn opt(v: Option<f64>) -> String {
    v.map(|p| p.to_string()).unwrap_or("-".into())
}
