//! Platforms that need the user's own session. Obsession never sees credentials: the login
//! command (AnyWeb's) opens the real site in a normal browser window, the person signs in there,
//! and the session stays in that tool's browser profile. Obsession only checks the profile exists.
use serde_json::{Value, json};
use std::path::PathBuf;

/// Where AnyWeb keeps browser profiles (`~/.web2api/profiles`, or `$WEB2API_HOME/profiles`).
fn profiles_dir() -> Option<PathBuf> {
    std::env::var_os("WEB2API_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".web2api")))
        .map(|h| h.join("profiles"))
}

/// A profile only counts once a browser has written into it.
fn profile_ready(name: &str) -> bool {
    profiles_dir()
        .map(|d| d.join(name))
        .and_then(|d| std::fs::read_dir(d).ok())
        .is_some_and(|mut entries| entries.next().is_some())
}

/// Every installed MCP server that declares a `login`, with whether its session is there.
pub fn list(registry: &Value) -> Vec<Value> {
    registry["servers"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, cfg)| {
            let login = cfg.get("login")?;
            let profile = login["profile"].as_str().unwrap_or(id);
            Some(json!({
                "id": id, "label": login["label"].as_str().unwrap_or(id), "site": login["site"],
                "connected": profile_ready(profile),
            }))
        })
        .collect()
}

/// True when the server needs no login or its session is in place.
pub fn usable(registry: &Value, server: &str) -> bool {
    let cfg = &registry["servers"][server];
    match cfg.get("login") {
        None => true,
        Some(login) => profile_ready(login["profile"].as_str().unwrap_or(server)),
    }
}

/// Opens the platform's own login flow in a new console window and returns at once; the
/// person finishes the login there. The command comes from an installed package, never from a model.
pub fn start_login(registry: &Value, server: &str) -> Result<(), String> {
    let login = registry["servers"][server].get("login").ok_or(format!("«{server}» no necesita login"))?;
    let command = login["command"].as_str().ok_or(format!("«{server}»: falta login.command en su paquete"))?;
    let args: Vec<&str> = login["args"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut cmd = std::process::Command::new(command);
    cmd.args(&args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // The login asks the person to press Enter, so it needs its own visible console.
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        cmd.creation_flags(CREATE_NEW_CONSOLE);
    }
    cmd.spawn().map(|_| ()).map_err(|e| format!("no se pudo abrir el login de {server}: {e}"))
}

/// Deletes the saved session for a platform. Only ever removes the profile folder it owns.
pub fn disconnect(registry: &Value, server: &str) -> Result<(), String> {
    let login = registry["servers"][server].get("login").ok_or(format!("«{server}» no tiene sesión"))?;
    let profile = login["profile"].as_str().unwrap_or(server);
    // Trust boundary: a profile name with separators could point outside the profiles folder.
    if profile.is_empty() || profile.contains(['/', '\\', '.']) {
        return Err(format!("nombre de perfil no válido: {profile}"));
    }
    let dir = profiles_dir().ok_or("no se encontró la carpeta de perfiles")?.join(profile);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("no se pudo borrar la sesión: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_and_guards_sessions() {
        let tmp = std::env::temp_dir().join(format!("obsession-conn-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("profiles/temu-chrome")).unwrap();
        std::fs::write(tmp.join("profiles/temu-chrome/Local State"), "{}").unwrap();
        // SAFETY: tests in this module are the only readers of WEB2API_HOME.
        unsafe { std::env::set_var("WEB2API_HOME", &tmp) };
        let reg = json!({"servers": {
            "amazon": {"command": "self"},
            "temu": {"command": "node", "login": {"label": "Temu", "profile": "temu-chrome"}},
            "ml": {"command": "node", "login": {"label": "ML", "profile": "ml-chrome"}},
            "bad": {"command": "node", "login": {"profile": "../x"}}
        }});
        assert!(usable(&reg, "amazon") && usable(&reg, "temu") && !usable(&reg, "ml"));
        let list = list(&reg);
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|c| c["id"] == "temu" && c["connected"] == true));
        assert!(disconnect(&reg, "bad").is_err());
        disconnect(&reg, "temu").unwrap();
        assert!(!usable(&reg, "temu"));
        std::fs::remove_dir_all(tmp).ok();
    }
}
