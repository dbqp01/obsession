//! BYOK: the person's own OpenAI-compatible providers. `providers.json` holds only names and URLs;
//! each key is a DPAPI-encrypted file under `keys/` that only the same Windows user can read.
//! A model written `byok:<provider>/<model>` goes to that provider; any other model goes to magpie.
use serde_json::{Value, json};
use std::path::PathBuf;

const PREFIX: &str = "byok:";

/// Where a chat request goes: base URL, bearer key (none for magpie) and the model id it expects.
pub struct Route {
    pub url: String,
    pub key: Option<String>,
    pub model: String,
}

fn dir() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|d| d.join("Obsession"))
        .ok_or("no se encontró LOCALAPPDATA".into())
}

/// Splits `byok:<provider>/<model>`; the model part may itself contain slashes.
pub fn parse_model(model: &str) -> Option<(&str, &str)> {
    let (provider, m) = model.strip_prefix(PREFIX)?.split_once('/')?;
    (valid_name(provider) && !m.is_empty()).then_some((provider, m))
}

/// Provider names become file names, so nothing that could leave the keys folder.
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn check_name(name: &str) -> Result<(), String> {
    if valid_name(name) { Ok(()) } else { Err(format!("nombre de proveedor no válido: «{name}» (usa letras, números, - o _)")) }
}

/// https only; plain http only to this machine. No user:password in the URL: keys go in `key set`.
pub fn validate_url(url: &str) -> Result<String, String> {
    let u = reqwest::Url::parse(url).map_err(|e| format!("URL no válida: {e}"))?;
    let host = u.host_str().ok_or("la URL no tiene host")?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    match u.scheme() {
        "https" => {}
        "http" if loopback => {}
        _ => return Err("la URL debe ser https (http solo para localhost)".into()),
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("no pongas credenciales en la URL; guarda la clave con `obsession key set`".into());
    }
    Ok(url.trim_end_matches('/').to_string())
}

fn load() -> Result<Value, String> {
    let path = dir()?.join("providers.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({"providers": {}})),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn add(name: &str, base_url: &str) -> Result<(), String> {
    check_name(name)?;
    let url = validate_url(base_url)?;
    let mut cfg = load()?;
    cfg["providers"][name] = json!({"base_url": url});
    let dir = dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("providers.json"), serde_json::to_string_pretty(&cfg).unwrap()).map_err(|e| e.to_string())
}

/// `(name, base_url, key stored)` for each provider; never the key itself.
pub fn list() -> Result<Vec<(String, String, bool)>, String> {
    let cfg = load()?;
    let keys = dir()?.join("keys");
    Ok(cfg["providers"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(n, p)| (n.clone(), p["base_url"].as_str().unwrap_or("").to_string(), keys.join(format!("{n}.key")).exists()))
        .collect())
}

fn key_path(name: &str) -> Result<PathBuf, String> {
    check_name(name)?;
    Ok(dir()?.join("keys").join(format!("{name}.key")))
}

pub fn set_key(name: &str, key: &str) -> Result<(), String> {
    let path = key_path(name)?;
    if load()?["providers"].get(name).is_none() {
        return Err(format!("no existe el proveedor «{name}»: agrégalo con `obsession provider add`"));
    }
    if key.trim().is_empty() {
        return Err("la clave está vacía".into());
    }
    let sealed = protect(key.trim().as_bytes())?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, sealed).map_err(|e| e.to_string())
}

pub fn remove_key(name: &str) -> Result<bool, String> {
    let path = key_path(name)?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn env_var(name: &str) -> String {
    format!("OBSESSION_KEY_{}", name.to_ascii_uppercase().replace('-', "_"))
}

/// The stored key, or `OBSESSION_KEY_<PROVIDER>` when none is stored.
fn key(name: &str) -> Result<String, String> {
    let path = key_path(name)?;
    match std::fs::read(&path) {
        Ok(sealed) => String::from_utf8(unprotect(&sealed)?).map_err(|_| format!("la clave de «{name}» está dañada")),
        Err(_) => std::env::var(env_var(name))
            .map_err(|_| format!("«{name}» no tiene clave: `obsession key set {name}` o {}", env_var(name))),
    }
}

pub fn route(model: &str) -> Result<Route, String> {
    let Some((provider, m)) = parse_model(model) else {
        if model.starts_with(PREFIX) {
            return Err(format!("modelo no válido: «{model}» (usa byok:<proveedor>/<modelo>)"));
        }
        let url = std::env::var("MAGPIE_URL").unwrap_or_else(|_| "http://127.0.0.1:3425/v1".into());
        return Ok(Route { url, key: None, model: model.into() });
    };
    let cfg = load()?;
    let base = cfg["providers"][provider]["base_url"]
        .as_str()
        .ok_or(format!("no existe el proveedor «{provider}» en providers.json"))?;
    // The file may have been edited by hand: check it again before a key goes there.
    Ok(Route { url: validate_url(base)?, key: Some(key(provider)?), model: m.into() })
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(data, true)
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    dpapi(data, false)
}

#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("en este sistema no se guardan claves (no hay DPAPI); usa OBSESSION_KEY_<PROVEEDOR>".into())
}

#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>, String> {
    Err("en este sistema no se pueden leer claves guardadas; usa OBSESSION_KEY_<PROVEEDOR>".into())
}

/// DPAPI in CurrentUser scope: only this Windows user on this machine can decrypt.
#[cfg(windows)]
fn dpapi(data: &[u8], seal: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    use std::ptr::{null, null_mut};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` borrows live bytes for the call; DPAPI allocates `out`, freed with LocalFree below.
    let ok = unsafe {
        if seal {
            CryptProtectData(&input, null(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, null_mut(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        }
    };
    if ok == 0 {
        return Err(format!("DPAPI: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: on success `out` holds cbData bytes owned by us until LocalFree.
    let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
    unsafe { LocalFree(out.pbData.cast()) };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_byok_models() {
        assert_eq!(parse_model("byok:openrouter/deepseek/deepseek-chat"), Some(("openrouter", "deepseek/deepseek-chat")));
        assert_eq!(parse_model("byok:deepseek/deepseek-chat"), Some(("deepseek", "deepseek-chat")));
        assert_eq!(parse_model("kiro/claude-haiku-5.5"), None, "magpie ids stay on magpie");
        assert_eq!(parse_model("byok:openai/"), None);
        assert_eq!(parse_model("byok:../x/m"), None);
        assert!(route("byok:sin-modelo").is_err());
        assert!(route("zcode/GLM-5.3-Flash").unwrap().key.is_none());
    }

    #[test]
    fn validates_base_urls() {
        assert_eq!(validate_url("https://api.deepseek.com/v1/").unwrap(), "https://api.deepseek.com/v1");
        assert!(validate_url("http://127.0.0.1:8080/v1").is_ok());
        assert!(validate_url("http://localhost/v1").is_ok());
        assert!(validate_url("http://[::1]:9000/v1").is_ok());
        assert!(validate_url("http://api.openai.com/v1").is_err());
        assert!(validate_url("http://localhost.evil.com/v1").is_err());
        assert!(validate_url("ftp://localhost/v1").is_err());
        assert!(validate_url("https://user:pw@api.openai.com/v1").is_err());
        assert!(validate_url("no es url").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let sealed = protect(b"test-value").unwrap();
        assert_ne!(sealed, b"test-value");
        assert_eq!(unprotect(&sealed).unwrap(), b"test-value");
        assert!(unprotect(b"garbage").is_err());
    }
}
