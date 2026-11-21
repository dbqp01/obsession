//! Packages bundle experts, skills and MCP servers behind one `obsession-package.json`.
//! Builtin ones live in `<root>/packages/`, user-installed ones in `%LOCALAPPDATA%\Obsession\packages\`.
//! Manifests are untrusted input: every name and path is checked before anything is read or copied.
use crate::agent::{self, Expert, Skill};
use crate::connections;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "obsession-package.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
    #[serde(default)]
    pub experts: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Map<String, Value>,
    #[serde(default)]
    pub requires: Vec<Requirement>,
    #[serde(default)]
    pub tested_with: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    /// `binary` (a program on PATH) or `session` (an MCP server's login).
    pub kind: String,
    pub name: String,
    pub why: String,
    /// binary only: `>=N` (major version from `<name> --version`) or a file path to look at when not on PATH.
    pub check: Option<String>,
}

pub struct Package {
    pub manifest: Manifest,
    pub builtin: bool,
    pub experts: Vec<Expert>,
    pub skills: Vec<Skill>,
}

pub struct Loaded {
    pub packages: Vec<Package>,
    /// Merged MCP servers in the shape `connections` and `agent` read: `{"servers": {...}}`.
    pub registry: Value,
    pub warnings: Vec<String>,
}

/// `%LOCALAPPDATA%\Obsession\packages`, where `obsession install` copies packages.
pub fn user_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Obsession").join("packages"))
}

pub fn load(root: &Path) -> Loaded {
    let mut dirs = vec![(root.join("packages"), true)];
    dirs.extend(user_dir().map(|d| (d, false)));
    load_dirs(&dirs)
}

/// Loads every package under each dir, in order. A package that is broken or reuses a name
/// already loaded is refused whole, with a warning: nothing is ever silently overridden.
pub fn load_dirs(dirs: &[(PathBuf, bool)]) -> Loaded {
    let mut packages: Vec<Package> = Vec::new();
    let mut warnings = Vec::new();
    for (dir, builtin) in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut paths: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).filter(|p| p.join(MANIFEST).is_file()).collect();
        paths.sort();
        for path in paths {
            match read(&path).and_then(|p| match clash(&packages, &p) {
                Some(c) => Err(format!("«{}» choca con un paquete ya cargado: {c}", p.manifest.name)),
                None => Ok(p),
            }) {
                Ok(p) => packages.push(Package { builtin: *builtin, ..p }),
                Err(e) => warnings.push(format!("paquete {} no se cargó: {e}", path.display())),
            }
        }
    }
    let servers: Map<String, Value> = packages.iter().flat_map(|p| p.manifest.mcp_servers.clone()).collect();
    Loaded { packages, registry: json!({"servers": servers}), warnings }
}

/// The first name `new` shares with an already loaded package, if any.
fn clash(loaded: &[Package], new: &Package) -> Option<String> {
    let pkgs: HashSet<&str> = loaded.iter().map(|p| p.manifest.name.as_str()).collect();
    let experts: HashSet<&str> = loaded.iter().flat_map(|p| &p.experts).map(|e| e.name.as_str()).collect();
    let skills: HashSet<&str> = loaded.iter().flat_map(|p| &p.skills).map(|s| s.name.as_str()).collect();
    let servers: HashSet<&str> = loaded.iter().flat_map(|p| p.manifest.mcp_servers.keys()).map(String::as_str).collect();
    let m = &new.manifest;
    if pkgs.contains(m.name.as_str()) {
        return Some(format!("paquete «{}»", m.name));
    }
    new.experts.iter().find(|e| experts.contains(e.name.as_str())).map(|e| format!("experto «{}»", e.name))
        .or_else(|| new.skills.iter().find(|s| skills.contains(s.name.as_str())).map(|s| format!("skill «{}»", s.name)))
        .or_else(|| m.mcp_servers.keys().find(|k| servers.contains(k.as_str())).map(|k| format!("MCP «{k}»")))
}

/// A name that is safe as a folder, a CLI argument and an MCP prefix.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// A path inside the package: relative, made only of plain components (no `..`, `.`, root or drive).
fn inside(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if rel.is_empty() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("ruta no permitida: «{rel}»"));
    }
    Ok(dir.join(p))
}

fn nonempty(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn expand_package_dir(cfg: &mut Value, dir: &str) {
    let fill = |v: &mut Value| {
        if let Some(s) = v.as_str() {
            *v = json!(s.replace("{package}", dir));
        }
    };
    // get_mut, not indexing: a manifest entry that is not an object must fail validation, not panic.
    if let Some(command) = cfg.get_mut("command") {
        fill(command);
    }
    if let Some(args) = cfg.get_mut("args").and_then(Value::as_array_mut) {
        args.iter_mut().for_each(fill);
    }
}

/// Reads and validates the package in `dir`, loading its experts and skills.
pub fn read(dir: &Path) -> Result<Package, String> {
    let path = dir.join(MANIFEST);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut m: Manifest = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    // `{package}` in a command or arg is this package's folder, so a bundled MCP (e.g. one AnyWeb
    // generated) can ship its own script and still run wherever the package was installed.
    let here = dir.to_string_lossy().replace('\\', "/");
    for cfg in m.mcp_servers.values_mut() {
        expand_package_dir(cfg, &here);
        if let Some(login) = cfg.get_mut("login") {
            expand_package_dir(login, &here);
        }
    }
    if !safe_name(&m.name) {
        return Err(format!("nombre de paquete no válido: «{}» (usa a-z, 0-9, - y _)", m.name));
    }
    if m.version.trim().is_empty() || m.source.trim().is_empty() {
        return Err("faltan version o source".into());
    }
    for (id, cfg) in &m.mcp_servers {
        if !safe_name(id) {
            return Err(format!("nombre de MCP no válido: «{id}»"));
        }
        if !nonempty(&cfg["command"]) {
            return Err(format!("MCP «{id}»: command vacío"));
        }
        if let Some(login) = cfg.get("login") {
            if !nonempty(&login["command"]) {
                return Err(format!("MCP «{id}»: login.command vacío"));
            }
        }
    }
    for r in &m.requires {
        if !safe_name(&r.name) {
            return Err(format!("requisito con nombre no válido: «{}»", r.name));
        }
        let ok = match r.kind.as_str() {
            "session" => m.mcp_servers.get(&r.name).is_some_and(|c| c.get("login").is_some()),
            "binary" => r.check.as_deref().and_then(|c| c.strip_prefix(">=")).is_none_or(|v| v.trim().parse::<u32>().is_ok()),
            _ => false,
        };
        if !ok {
            return Err(format!("requisito no válido: {} «{}»", r.kind, r.name));
        }
    }
    let experts = m.experts.iter().map(|rel| agent::load_expert(&inside(dir, rel)?)).collect::<Result<Vec<_>, _>>()?;
    for e in &experts {
        if !safe_name(&e.name) {
            return Err(format!("nombre de experto no válido: «{}»", e.name));
        }
        // An expert reaches only its own package's servers; anything else would depend on install order.
        if let Some(s) = e.mcp.iter().find(|s| !m.mcp_servers.contains_key(*s)) {
            return Err(format!("el experto {} usa el MCP «{s}», que no está en el paquete", e.name));
        }
    }
    let skills = m.skills.iter().map(|rel| agent::load_skill(&inside(dir, rel)?)).collect::<Result<Vec<_>, _>>()?;
    Ok(Package { manifest: m, builtin: false, experts, skills })
}

/// What the package still needs on this machine, as Spanish instructions. Empty means ready.
pub fn unmet(m: &Manifest, registry: &Value) -> Vec<String> {
    m.requires
        .iter()
        .filter_map(|r| match r.kind.as_str() {
            "session" if !connections::usable(registry, &r.name) => {
                let label = registry["servers"][&r.name]["login"]["label"].as_str().unwrap_or(&r.name);
                Some(format!("falta iniciar sesión en {label}: obsession login {}", r.name))
            }
            "binary" if !binary_ok(&r.name, r.check.as_deref()) => {
                let need = r.check.as_deref().filter(|c| c.starts_with(">=")).map(|c| format!(" {c}")).unwrap_or_default();
                Some(format!("falta {}{need}: {}", r.name, r.why))
            }
            _ => None,
        })
        .collect()
}

/// Every skill, plus the experts (sorted) of packages with nothing unmet; the others get a Spanish warning each.
pub fn ready(packages: Vec<Package>, registry: &Value) -> (Vec<Expert>, Vec<Skill>, Vec<String>) {
    let (mut experts, mut skills, mut warnings) = (Vec::new(), Vec::new(), Vec::new());
    for p in packages {
        let todo = unmet(&p.manifest, registry);
        skills.extend(p.skills);
        if todo.is_empty() {
            experts.extend(p.experts);
        } else {
            warnings.extend(p.experts.iter().map(|e| format!("{} no participa: {}", e.name, todo.join("; "))));
        }
    }
    experts.sort_by(|a, b| a.name.cmp(&b.name));
    (experts, skills, warnings)
}

fn binary_ok(name: &str, check: Option<&str>) -> bool {
    match check.and_then(|c| c.strip_prefix(">=")) {
        Some(min) => major_version(name).zip(min.trim().parse().ok()).is_some_and(|(v, min): (u32, u32)| v >= min),
        None => on_path(name) || check.is_some_and(|p| Path::new(p).is_file()),
    }
}

fn on_path(name: &str) -> bool {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or(".EXE;.CMD;.BAT".into()).split(';').map(str::to_string).collect()
    } else {
        vec![String::new()]
    };
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| exts.iter().any(|x| d.join(format!("{name}{x}")).is_file())))
}

/// First number in `<name> --version` (e.g. `v24.22.1` → 24).
fn major_version(name: &str) -> Option<u32> {
    let out = std::process::Command::new(name).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty())?.parse().ok()
}

/// Validates the package at `src` and copies it to `user/<name>`. Returns its name and what is still missing.
pub fn install(src: &Path, builtin: &Path, user: &Path) -> Result<(String, Vec<String>), String> {
    let new = read(src)?;
    let existing = load_dirs(&[(builtin.to_path_buf(), true), (user.to_path_buf(), false)]);
    if let Some(c) = clash(&existing.packages, &new) {
        return Err(format!("ya hay un {c} instalado; desinstálalo primero"));
    }
    let name = new.manifest.name.clone();
    let dest = user.join(&name);
    if dest.exists() {
        return Err(format!("{} ya existe", dest.display()));
    }
    // The copy is read again: symlinks are not copied, so what loads is what got installed.
    if let Err(e) = copy_dir(src, &dest).map_err(|e| e.to_string()).and_then(|_| read(&dest).map(|_| ())) {
        std::fs::remove_dir_all(&dest).ok();
        return Err(format!("no se pudo instalar: {e}"));
    }
    let after = load_dirs(&[(builtin.to_path_buf(), true), (user.to_path_buf(), false)]);
    let todo = after.packages.iter().find(|p| p.manifest.name == name).map(|p| unmet(&p.manifest, &after.registry));
    Ok((name, todo.unwrap_or_default()))
}

/// Marketplace packages are npm packages under this prefix. Only this scope is trusted for now:
/// a package runs its MCP on the person's machine, and only the owner publishes there.
pub const NPM_PREFIX: &str = "@akimibks/obsession-";

/// `@akimibks/obsession-<name>` with an optional exact `@x.y.z`. Anything else is refused before npm runs.
pub fn npm_spec_ok(spec: &str) -> bool {
    let Some(rest) = spec.strip_prefix(NPM_PREFIX) else { return false };
    let (name, version) = rest.split_once('@').unwrap_or((rest, ""));
    let name_ok = !name.is_empty() && name.len() <= 50 && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let version_ok = version.is_empty() || {
        let parts: Vec<&str> = version.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    };
    name_ok && version_ok
}

/// Downloads a marketplace package with `npm pack`, which checks the registry's sha512 integrity,
/// unpacks it and installs it like a local folder. The temporary copy is always removed.
pub fn install_npm(spec: &str, builtin: &Path, user: &Path) -> Result<(String, Vec<String>), String> {
    if !npm_spec_ok(spec) {
        return Err(format!("solo se instalan paquetes {NPM_PREFIX}<nombre>[@x.y.z]; «{spec}» no lo es"));
    }
    let work = std::env::temp_dir().join(format!("obsession-npm-{}-{}", std::process::id(), stamp()));
    std::fs::create_dir_all(&work).map_err(|e| format!("no se pudo crear {}: {e}", work.display()))?;
    let result = npm_pack(spec, &work).and_then(|tgz| unpack(&tgz, &work)).and_then(|dir| install(&dir, builtin, user));
    std::fs::remove_dir_all(&work).ok();
    result
}

fn stamp() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

fn npm_pack(spec: &str, dest: &Path) -> Result<PathBuf, String> {
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let out = std::process::Command::new(npm)
        .args(["pack", spec, "--json", "--pack-destination"])
        .arg(dest)
        .output()
        .map_err(|e| format!("no se pudo ejecutar npm ({e}); instala Node.js 24"))?;
    // With --json npm reports failures on stdout as {"error": {"code", "summary"}}.
    let packed: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !out.status.success() || packed.get("error").is_some() {
        return Err(match packed["error"]["code"].as_str() {
            Some("E404") => format!("{spec} no existe en npm"),
            Some(code) => format!("no se pudo descargar {spec}: npm {code}"),
            None => format!("no se pudo descargar {spec}: npm falló"),
        });
    }
    let file = packed[0]["filename"].as_str().ok_or("npm no dijo qué archivo descargó")?;
    // npm names the tarball itself; it must stay a plain file name inside `dest`.
    if file.contains(['/', '\\']) || file.contains("..") {
        return Err(format!("nombre de archivo inesperado de npm: {file}"));
    }
    Ok(dest.join(file))
}

/// Unpacks an npm tarball and returns its `package/` folder. On Windows the system bsdtar is used by
/// full path; it refuses `..` and absolute entries, and `read` validates the manifest afterwards.
fn unpack(tgz: &Path, dest: &Path) -> Result<PathBuf, String> {
    let tar = match std::env::var_os("SystemRoot") {
        Some(root) if cfg!(windows) => PathBuf::from(root).join("System32").join("tar.exe"),
        _ => PathBuf::from("tar"),
    };
    let out_dir = dest.join("unpacked");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let status = std::process::Command::new(&tar).arg("-xzf").arg(tgz).arg("-C").arg(&out_dir).status()
        .map_err(|e| format!("no se pudo ejecutar tar: {e}"))?;
    if !status.success() {
        return Err("el paquete descargado no se pudo descomprimir".into());
    }
    let dir = out_dir.join("package");
    if !dir.join(MANIFEST).is_file() {
        return Err(format!("el paquete npm no trae {MANIFEST}"));
    }
    Ok(dir)
}

/// Searches the npm registry for marketplace packages. Results outside the trusted prefix are dropped.
pub fn search(text: &str) -> Result<Vec<(String, String, String)>, String> {
    let query = format!("keywords:obsession-package {text}");
    let url = reqwest::Url::parse_with_params("https://registry.npmjs.org/-/v1/search", [("text", query.trim()), ("size", "50")])
        .map_err(|e| e.to_string())?;
    let body: Value = reqwest::blocking::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| format!("no se pudo buscar en npm: {e}"))?;
    Ok(body["objects"].as_array().into_iter().flatten().filter_map(|o| {
        let p = &o["package"];
        let name = p["name"].as_str()?;
        npm_spec_ok(name).then(|| (name.to_string(), s(&p["version"]), s(&p["description"])))
    }).collect())
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Removes a user-installed package. Builtin packages ship with Obsession and are refused.
pub fn uninstall(name: &str, builtin: &Path, user: &Path) -> Result<(), String> {
    if !safe_name(name) {
        return Err(format!("nombre de paquete no válido: «{name}»"));
    }
    let dir = user.join(name);
    if dir.join(MANIFEST).is_file() {
        return std::fs::remove_dir_all(&dir).map_err(|e| format!("no se pudo borrar {}: {e}", dir.display()));
    }
    if builtin.join(name).join(MANIFEST).is_file() {
        return Err(format!("«{name}» viene con Obsession y no se desinstala"));
    }
    Err(format!("«{name}» no está instalado"))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = to.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPERT: &str = "---\nname: NAME\ndescription: d\nmodel: m\nmcp: NAME\nskills: report-findings\n---\nEres el experto.";

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("obsession-pkg-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A one-expert package `name` under `dir`, with `extra` merged into the manifest.
    fn package(dir: &Path, name: &str, extra: Value) -> PathBuf {
        let p = dir.join(name);
        std::fs::create_dir_all(p.join("experts")).unwrap();
        std::fs::write(p.join("experts/e.md"), EXPERT.replace("NAME", name)).unwrap();
        let mut m = json!({"name": name, "version": "1.0.0", "description": "d", "source": "anyweb",
            "experts": ["experts/e.md"], "mcp_servers": {name: {"command": "node", "args": ["x.mjs"]}}});
        for (k, v) in extra.as_object().unwrap() {
            m[k] = v.clone();
        }
        std::fs::write(p.join(MANIFEST), m.to_string()).unwrap();
        p
    }

    #[test]
    fn package_placeholder_points_at_its_own_folder() {
        let mut cfg = json!({"command": "node", "args": ["{package}/mcp/server.mjs", "--flag"],
            "login": {"command": "{package}/login.cmd", "args": []}});
        expand_package_dir(&mut cfg, "C:/pkgs/hiraoka");
        if let Some(login) = cfg.get_mut("login") {
            expand_package_dir(login, "C:/pkgs/hiraoka");
        }
        assert_eq!(cfg["args"][0], "C:/pkgs/hiraoka/mcp/server.mjs");
        assert_eq!(cfg["login"]["command"], "C:/pkgs/hiraoka/login.cmd");
        let mut odd = json!("not an object");
        expand_package_dir(&mut odd, "x"); // must not panic
    }

    #[test]
    fn repo_packages_load_every_builtin_expert() {
        let l = load_dirs(&[(Path::new(env!("CARGO_MANIFEST_DIR")).join("packages"), true)]);
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        let mut names: Vec<&str> = l.packages.iter().flat_map(|p| &p.experts).map(|e| e.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["aliexpress", "amazon", "coolbox", "estilos", "falabella", "knasta", "oechsle", "plazavea", "promart", "ripley", "temu"]);
        let skills: Vec<&str> = l.packages.iter().flat_map(|p| &p.skills).map(|s| s.name.as_str()).collect();
        for e in l.packages.iter().flat_map(|p| &p.experts) {
            assert!(e.skills.iter().all(|s| skills.contains(&s.as_str())), "{} pide una skill que no existe", e.name);
            assert!(e.mcp.iter().all(|m| l.registry["servers"].get(m).is_some()));
        }
        assert!(l.registry["servers"]["temu"]["login"]["profile"] == "temu-chrome");
    }

    #[test]
    fn manifest_validation() {
        let d = tmp("valid");
        assert!(read(&package(&d, "good", json!({}))).is_ok());
        let bad = [
            (json!({"name": "../evil"}), "nombre de paquete"),
            (json!({"name": r"a\b"}), "nombre de paquete"),
            (json!({"name": "a/b"}), "nombre de paquete"),
            (json!({"name": "Upper"}), "nombre de paquete"),
            (json!({"experts": ["C:/Windows/win.ini"]}), "ruta no permitida"),
            (json!({"experts": ["/etc/passwd"]}), "ruta no permitida"),
            (json!({"experts": ["../outside.md"]}), "ruta no permitida"),
            (json!({"skills": ["skills/../../x"]}), "ruta no permitida"),
            (json!({"skills": ["./skills/x"]}), "ruta no permitida"),
            (json!({"experts": ["experts/nope.md"]}), "nope.md"),
            (json!({"mcp_servers": {"x": {"command": " "}}}), "command vacío"),
            (json!({"mcp_servers": {"x": {"args": ["a"]}}}), "command vacío"),
            (json!({"mcp_servers": {"x": {"command": "node", "login": {"label": "X"}}}}), "login.command vacío"),
            (json!({"mcp_servers": {"x": {"command": "node"}, "a/b": {"command": "node"}}}), "nombre de MCP"),
            (json!({"mcp_servers": {"y": {"command": "node"}}}), "no está en el paquete"),
            (json!({"requires": [{"kind": "magic", "name": "x", "why": "y"}]}), "requisito no válido"),
            (json!({"requires": [{"kind": "binary", "name": "node", "check": ">=v24", "why": "y"}]}), "requisito no válido"),
            (json!({"requires": [{"kind": "session", "name": "x", "why": "y"}]}), "requisito no válido"),
            (json!({"requires": [{"kind": "binary", "name": "../node", "why": "y"}]}), "requisito con nombre"),
            (json!({"license": "MIT"}), "unknown field"),
            (json!({"version": ""}), "faltan version"),
        ];
        for (i, (extra, why)) in bad.iter().enumerate() {
            // Each case is a valid package "x" with one field broken.
            let err = read(&package(&d.join(i.to_string()), "x", extra.clone())).err().unwrap_or_default();
            assert!(err.contains(why), "caso {i}: esperaba «{why}», salió «{err}»");
        }
        let p = package(&d, "broken", json!({}));
        std::fs::write(p.join("experts/e.md"), "sin frontmatter").unwrap();
        assert!(read(&p).err().unwrap().contains("frontmatter"));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn merge_refuses_collisions() {
        let (a, b) = (tmp("merge-a"), tmp("merge-b"));
        package(&a, "shop", json!({}));
        package(&b, "shop", json!({})); // same package name
        let p = package(&b, "other", json!({}));
        std::fs::write(p.join("experts/e.md"), EXPERT.replace("NAME", "shop").replace("mcp: shop", "mcp: other")).unwrap();
        package(&b, "fresh", json!({}));
        let l = load_dirs(&[(a.clone(), true), (b.clone(), false)]);
        let names: Vec<&str> = l.packages.iter().map(|p| p.manifest.name.as_str()).collect();
        assert_eq!(names, ["shop", "fresh"]);
        assert!(l.packages[0].builtin && !l.packages[1].builtin);
        assert_eq!(l.warnings.len(), 2, "{:?}", l.warnings);
        assert!(l.warnings.iter().any(|w| w.contains("experto «shop»")));
        assert!(l.registry["servers"].get("fresh").is_some() && l.registry["servers"].get("other").is_none());
        std::fs::remove_dir_all(a).ok();
        std::fs::remove_dir_all(b).ok();
    }

    #[test]
    fn unmet_requirements() {
        let d = tmp("req");
        let p = package(&d, "req", json!({
            "mcp_servers": {"req": {"command": "node", "login": {"label": "Req", "profile": "obsession-test-no-such-profile", "command": "node"}}},
            "requires": [
                {"kind": "binary", "name": "obsession-no-such-binary", "why": "hace falta"},
                {"kind": "binary", "name": "obsession-no-such-binary", "check": "/no/such/file", "why": "hace falta"},
                {"kind": "session", "name": "req", "why": "precios con cuenta"}
            ]}));
        let pkg = read(&p).unwrap();
        let reg = json!({"servers": pkg.manifest.mcp_servers});
        let todo = unmet(&pkg.manifest, &reg);
        assert_eq!(todo.len(), 3, "{todo:?}");
        assert!(todo[2].contains("obsession login req"));
        // A check path that exists satisfies a binary that is not on PATH.
        assert!(binary_ok("obsession-no-such-binary", Some(p.join(MANIFEST).to_str().unwrap())));
        let free = read(&package(&d, "free", json!({}))).unwrap();
        assert!(unmet(&free.manifest, &reg).is_empty());
        let (experts, _, warnings) = ready(vec![pkg, free], &reg);
        assert_eq!(experts.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["free"]);
        assert!(warnings.len() == 1 && warnings[0].starts_with("req no participa: falta obsession-no-such-binary"));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn install_uninstall_round_trip() {
        let (src, builtin, user) = (tmp("src"), tmp("builtin"), tmp("user"));
        package(&builtin, "core", json!({}));
        let p = package(&src, "nuevo", json!({}));
        std::fs::write(p.join("x.mjs"), "// server").unwrap();
        let (name, todo) = install(&p, &builtin, &user).unwrap();
        assert_eq!(name, "nuevo");
        assert!(todo.is_empty());
        assert!(user.join("nuevo/x.mjs").is_file());
        assert!(install(&p, &builtin, &user).is_err(), "instalar dos veces");
        assert!(install(&builtin.join("core"), &builtin, &user).is_err(), "choca con uno de serie");
        let l = load_dirs(&[(builtin.clone(), true), (user.clone(), false)]);
        assert_eq!(l.packages.len(), 2);
        assert!(uninstall("core", &builtin, &user).is_err(), "los de serie no se desinstalan");
        assert!(uninstall("../builtin", &builtin, &user).is_err());
        uninstall("nuevo", &builtin, &user).unwrap();
        assert!(!user.join("nuevo").exists());
        assert!(uninstall("nuevo", &builtin, &user).is_err());
        for d in [src, builtin, user] {
            std::fs::remove_dir_all(d).ok();
        }
    }

    #[test]
    fn npm_specs_stay_in_the_trusted_scope() {
        for ok in ["@akimibks/obsession-hiraoka", "@akimibks/obsession-temu-pe@1.2.30"] {
            assert!(npm_spec_ok(ok), "{ok}");
        }
        for bad in ["hiraoka", "@otro/obsession-x", "@akimibks/anyweb", "@akimibks/obsession-", "@akimibks/obsession-X",
            "@akimibks/obsession-x@latest", "@akimibks/obsession-x@1.2", "@akimibks/obsession-x@^1.0.0", "@akimibks/obsession-x --global",
            "@akimibks/obsession-../x", "@akimibks/obsession-x@1.0.0@2.0.0"] {
            assert!(!npm_spec_ok(bad), "{bad}");
        }
        let (builtin, user) = (tmp("npm-builtin"), tmp("npm-user"));
        assert!(install_npm("@otro/obsession-x", &builtin, &user).unwrap_err().contains("solo se instalan"));
    }

    #[test]
    fn unpacks_an_npm_tarball_and_installs_it() {
        let (work, builtin, user) = (tmp("tgz"), tmp("tgz-builtin"), tmp("tgz-user"));
        let staged = work.join("stage");
        let p = package(&staged, "package", json!({}));
        std::fs::write(p.join("x.mjs"), "// bundle").unwrap();
        // npm tarballs keep everything under `package/`; build one the same way.
        let tgz = work.join("pkg.tgz");
        let tar = match std::env::var_os("SystemRoot") {
            Some(root) if cfg!(windows) => PathBuf::from(root).join("System32").join("tar.exe"),
            _ => PathBuf::from("tar"),
        };
        assert!(std::process::Command::new(tar).arg("-czf").arg(&tgz).arg("-C").arg(&staged).arg("package").status().unwrap().success());
        let dir = unpack(&tgz, &work).unwrap();
        let (name, _) = install(&dir, &builtin, &user).unwrap();
        assert_eq!(name, "package");
        assert!(user.join("package/x.mjs").is_file());
        assert!(unpack(&work.join("missing.tgz"), &work).is_err());
        for d in [work, builtin, user] {
            std::fs::remove_dir_all(d).ok();
        }
    }
}
