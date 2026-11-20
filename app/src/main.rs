#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Only one search at a time: experts share the router and the stores.
struct Busy(AtomicBool);

/// shortcut: builtin packages/ are read from the repo this app was built in;
/// set OBSESSION_HOME or bundle them as resources before shipping to other machines.
fn root() -> PathBuf {
    std::env::var_os("OBSESSION_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")))
}

#[tauri::command]
fn search(app: AppHandle, query: String) -> Result<(), String> {
    let query = query.trim().to_string();
    if query.len() < 5 {
        return Err("Describe qué quieres comprar, tu presupuesto y tu ciudad.".into());
    }
    if app.state::<Busy>().0.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una búsqueda en curso.".into());
    }
    std::thread::spawn(move || {
        let emit = |event: serde_json::Value| {
            let _ = app.emit("obsession", event);
        };
        if let Err(e) = obsession::run(&root(), &query, &emit) {
            emit(serde_json::json!({"kind": "error", "text": e.to_string()}));
        }
        app.state::<Busy>().0.store(false, Ordering::SeqCst);
    });
    Ok(())
}

fn registry() -> Result<serde_json::Value, String> {
    obsession::load_registry(&root()).map_err(|e| e.to_string())
}

#[tauri::command]
fn connections() -> Result<Vec<serde_json::Value>, String> {
    Ok(obsession::connections::list(&registry()?))
}

/// Opens the platform's own login in a new console; the person signs in there, never here.
#[tauri::command]
fn connect(id: String) -> Result<(), String> {
    obsession::connections::start_login(&registry()?, &id)
}

#[tauri::command]
fn disconnect(id: String) -> Result<(), String> {
    obsession::connections::disconnect(&registry()?, &id)
}

fn main() {
    // Packages start store servers as "self": answer them before opening any window.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("mcp") {
        if let Err(e) = obsession::mcp::serve(args.get(1).map(String::as_str).unwrap_or("")) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Busy(AtomicBool::new(false)))
        .invoke_handler(tauri::generate_handler![search, connections, connect, disconnect])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir la ventana de Obsession");
}
