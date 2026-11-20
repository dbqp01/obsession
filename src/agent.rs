//! Experts are agents defined in `.md` files; skills are standard `SKILL.md` dirs. Both ship in packages.
//! Only the orchestrator invokes an expert, and only the expert sees its own MCP tools.
use crate::{Emit, llm, mcp, stores::Listing};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

const HARNESS: &str = include_str!("../prompts/expert_harness.md");
const TABLE_ROWS: usize = 20;
const RAW_TEXT_CHARS: usize = 1500;

#[derive(Debug)]
pub struct Expert {
    pub name: String,
    pub description: String,
    pub model: String,
    pub mcp: Vec<String>,
    pub skills: Vec<String>,
    pub max_steps: usize,
    pub body: String,
}

#[derive(Debug)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Splits `---\nkey: value\n---\nbody` into its fields and body.
fn frontmatter(text: &str) -> Option<(HashMap<String, String>, String)> {
    let rest = text.trim_start_matches('\u{feff}').strip_prefix("---")?;
    let (head, body) = rest.split_once("\n---")?;
    let fields = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    Some((fields, body.trim_start_matches(['-', '\r', '\n']).trim().to_string()))
}

fn list(v: Option<&String>) -> Vec<String> {
    v.map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default()
}

fn read_md(path: &std::path::Path) -> Result<(HashMap<String, String>, String), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    frontmatter(&text).ok_or(format!("{}: falta el frontmatter ---", path.display()))
}

pub fn load_expert(path: &std::path::Path) -> Result<Expert, String> {
    let (f, body) = read_md(path)?;
    let need = |k: &str| f.get(k).cloned().ok_or(format!("{}: falta `{k}`", path.display()));
    Ok(Expert {
        name: need("name")?,
        description: need("description")?,
        model: need("model")?,
        mcp: list(f.get("mcp")),
        skills: list(f.get("skills")),
        max_steps: f.get("max_steps").and_then(|s| s.parse().ok()).unwrap_or(6).max(2),
        body,
    })
}

/// A skill directory holding a standard `SKILL.md`.
pub fn load_skill(dir: &std::path::Path) -> Result<Skill, String> {
    let path = dir.join("SKILL.md");
    let (f, body) = read_md(&path)?;
    let (Some(name), Some(description)) = (f.get("name"), f.get("description")) else {
        return Err(format!("{}: falta name o description", path.display()));
    };
    Ok(Skill { name: name.clone(), description: description.clone(), body })
}

/// Every listing any expert saw this run, addressed by a stable id. Tool results go here
/// instead of into the model's context.
#[derive(Default)]
pub struct Store {
    listings: Mutex<Vec<Listing>>,
}

impl Store {
    /// Adds listings (deduped by URL) and returns their ids.
    pub fn add(&self, new: Vec<Listing>) -> Vec<usize> {
        let mut all = self.listings.lock().unwrap();
        new.into_iter()
            .map(|l| match all.iter().position(|x| x.url == l.url) {
                Some(i) => i,
                None => {
                    all.push(l);
                    all.len() - 1
                }
            })
            .collect()
    }

    pub fn get(&self, id: usize) -> Option<Listing> {
        self.listings.lock().unwrap().get(id).cloned()
    }

    pub fn all(&self) -> Vec<Listing> {
        self.listings.lock().unwrap().clone()
    }

    /// One line per listing: what the model needs to choose, nothing more.
    pub fn table(&self, ids: &[usize]) -> String {
        let all = self.listings.lock().unwrap();
        let mut lines: Vec<String> = ids
            .iter()
            .take(TABLE_ROWS)
            .filter_map(|&id| all.get(id).map(|l| (id, l)))
            .map(|(id, l)| {
                let title: String = l.title.chars().take(90).collect();
                let rating = l.rating.map(|r| format!("{r:.1}")).unwrap_or("-".into());
                format!("#{id} | {} | S/{:.2} | {rating} | {} | {title}", l.store, l.price_pen, l.sold.as_deref().unwrap_or("-"))
            })
            .collect();
        if ids.len() > TABLE_ROWS {
            lines.push(format!("({} resultados más omitidos; afina la búsqueda)", ids.len() - TABLE_ROWS));
        }
        if lines.is_empty() { "Sin resultados.".into() } else { lines.join("\n") }
    }
}

fn function(name: &str, description: &str, parameters: Value) -> Value {
    json!({"type": "function", "function": {"name": name, "description": description, "parameters": parameters}})
}

fn submit_tool() -> Value {
    function(
        "submit",
        "Entrega final al orquestador. Termina tu trabajo.",
        json!({"type": "object", "required": ["picks", "tried", "missing"], "properties": {
            "picks": {"type": "array", "items": {"type": "object", "required": ["id", "why"],
                "properties": {"id": {"type": "integer"}, "why": {"type": "string"}}}},
            "tried": {"type": "array", "items": {"type": "string"}},
            "missing": {"type": "string"}
        }}),
    )
}

fn failed(reason: String) -> Value {
    json!({"picks": [], "tried": [], "missing": reason})
}

/// Runs one expert to completion. Always returns a submission, even on failure, so the
/// orchestrator never waits on an expert that gave up.
pub fn run_expert(expert: &Expert, task: &str, registry: &Value, skills: &[Skill], store: &Store, emit: Emit) -> Value {
    let name = expert.name.as_str();
    let note = |text: String| emit(json!({"kind": "expert_note", "expert": name, "text": text}));

    // Hands: start this expert's MCP servers and namespace their tools as server__tool.
    let mut clients: HashMap<String, mcp::Client> = HashMap::new();
    let mut tools = Vec::new();
    for server in &expert.mcp {
        let Some(cfg) = registry["servers"].get(server) else {
            note(format!("MCP «{server}» no está en ningún paquete instalado"));
            continue;
        };
        match mcp::Client::from_config(cfg).and_then(|mut c| c.list_tools().map(|t| (c, t))) {
            Ok((client, declared)) => {
                // The harness re-opens pages itself; experts spend their steps searching.
                for t in declared.into_iter().filter(|t| t["name"] != "open") {
                    let name = format!("{server}__{}", t["name"].as_str().unwrap_or("tool"));
                    tools.push(function(&name, t["description"].as_str().unwrap_or(""), t["inputSchema"].clone()));
                }
                clients.insert(server.clone(), client);
            }
            Err(e) => note(format!("MCP «{server}» no arrancó: {e}")),
        }
    }
    if clients.is_empty() {
        return failed("ningún MCP del experto arrancó".into());
    }

    // Progressive disclosure: the prompt lists skill names; bodies load on demand.
    let my_skills: Vec<&Skill> = skills.iter().filter(|s| expert.skills.contains(&s.name)).collect();
    tools.push(function("inspect", "Detalle completo de un listing por id.",
        json!({"type": "object", "required": ["id"], "properties": {"id": {"type": "integer"}}})));
    if !my_skills.is_empty() {
        tools.push(function("load_skill", "Carga las instrucciones completas de una skill.",
            json!({"type": "object", "required": ["name"], "properties": {"name": {"type": "string",
                "enum": my_skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>()}}})));
    }
    tools.push(submit_tool());
    let skill_menu: String = my_skills.iter().map(|s| format!("- {}: {}\n", s.name, s.description)).collect();
    let system = format!("{HARNESS}\n\n{}\n\nSkills disponibles:\n{skill_menu}", expert.body);
    let mut messages = vec![json!({"role": "system", "content": system}), json!({"role": "user", "content": task})];
    let mut seen: Vec<usize> = Vec::new();
    let mut blocked = false;

    for step in 0..expert.max_steps {
        // A store that blocks stays blocked: more searches only burn steps.
        let last = blocked || step + 1 == expert.max_steps;
        if last {
            let why = if blocked { "La tienda bloqueó las búsquedas." } else { "Último paso." };
            messages.push(json!({"role": "user", "content": format!("{why} Llama a submit ahora con lo que tienes.")}));
        }
        let offered = if last { vec![submit_tool()] } else { tools.clone() };
        let msg = match llm::chat_tools(&expert.model, &messages, &offered, last) {
            Ok(m) => m,
            Err(e) => return failed(format!("el modelo falló: {e}")),
        };
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        // Echo only what the API needs; reasoning text would only grow the context.
        messages.push(json!({"role": "assistant", "content": msg["content"].as_str().unwrap_or(""), "tool_calls": calls}));
        if calls.is_empty() {
            // Indecision guard: prose is not an action.
            messages.pop();
            messages.push(json!({"role": "assistant", "content": msg["content"].as_str().unwrap_or("")}));
            messages.push(json!({"role": "user", "content": "Responde con una llamada a tool. Si ya decidiste, llama a submit."}));
            continue;
        }
        for call in calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap_or("{}")).unwrap_or(json!({}));
            if name == "submit" {
                let mut out = if args.is_object() { args } else { failed("submit sin objeto".into()) };
                // Drop ids the expert never saw: it may not invent listings.
                if let Some(picks) = out["picks"].as_array_mut() {
                    picks.retain(|p| p["id"].as_u64().is_some_and(|id| seen.contains(&(id as usize))));
                }
                out["steps"] = json!(step + 1);
                return out;
            }
            let result = match name {
                "inspect" => args["id"]
                    .as_u64()
                    .and_then(|id| store.get(id as usize))
                    .map(|l| serde_json::to_string(&l).unwrap())
                    .unwrap_or("id desconocido".into()),
                "load_skill" => my_skills
                    .iter()
                    .find(|s| args["name"] == s.name.as_str())
                    .map(|s| s.body.clone())
                    .unwrap_or("skill desconocida".into()),
                _ => match name.split_once("__").and_then(|(srv, tool)| clients.get_mut(srv).map(|c| (c, tool))) {
                    None => format!("tool desconocida: {name}"),
                    Some((client, tool)) => match client.call(tool, args.clone()) {
                        Err(e) => {
                            // Surface it: otherwise a failing source just looks like "0 picks".
                            note(format!("{tool} falló: {e}"));
                            blocked |= e.contains(crate::stores::BLOCKED);
                            format!("error: {e}")
                        }
                        Ok(text) => match serde_json::from_str::<Vec<Listing>>(&text)
                            .ok()
                            .or_else(|| text_listings(&text, name.split_once("__").map_or(name, |(srv, _)| srv)))
                        {
                            Some(listings) => {
                                let ids = store.add(listings);
                                emit(json!({"kind": "search", "expert": expert.name, "query": args["query"], "results": ids.len()}));
                                seen.extend(&ids);
                                store.table(&ids)
                            }
                            // shortcut: other non-listing MCP output is truncated, not summarized; summarize when such MCPs join.
                            None => text.chars().take(RAW_TEXT_CHARS).collect(),
                        },
                    },
                },
            };
            messages.push(json!({"role": "tool", "tool_call_id": call["id"], "content": result}));
        }
    }
    failed("se agotaron los pasos sin submit".into())
}

/// Reads AnyWeb's text format: numbered "N. title" blocks with "Precio:" and "Enlace:" lines.
fn text_listings(text: &str, store: &str) -> Option<Vec<Listing>> {
    let mut out = Vec::new();
    let mut cur: Option<(String, Option<f64>, Option<String>)> = None;
    let flush = |c: Option<(String, Option<f64>, Option<String>)>, out: &mut Vec<Listing>| {
        if let Some((title, Some(price), Some(url))) = c {
            out.push(Listing { store: store.into(), title, price_pen: price, url, seller: None, rating: None, sold: None });
        }
    };
    for line in text.lines() {
        let t = line.trim();
        if let Some((_, title)) = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            flush(cur.take(), &mut out);
            cur = Some((title.to_string(), None, None));
        } else if let Some(c) = cur.as_mut() {
            if let Some(p) = t.strip_prefix("Precio:") {
                c.1 = Some(crate::stores::parse_price(p)).filter(|p| *p > 0.0);
            } else if let Some(u) = t.strip_prefix("Enlace:") {
                c.2 = Some(u.trim().to_string());
            }
        }
    }
    flush(cur, &mut out);
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_anyweb_text_listings() {
        let text = "1. CCA CRA IEM\n   Precio: S/ 24.49\n   Imagen: x\n   Enlace: https://www.temu.com/pe/a.html\n\n\
                    2. Sin precio\n   Precio: No mostrado\n   Enlace: https://www.temu.com/pe/b.html";
        let l = text_listings(text, "temu").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].title.as_str(), l[0].price_pen, l[0].store.as_str()), ("CCA CRA IEM", 24.49, "temu"));
        assert!(text_listings("No encontré productos.", "temu").is_none());
    }

    #[test]
    fn parses_frontmatter() {
        let (f, body) = frontmatter("---\nname: amazon\nmcp: amazon, temu\n---\nEres el experto.").unwrap();
        assert_eq!(f["name"], "amazon");
        assert_eq!(list(f.get("mcp")), ["amazon", "temu"]);
        assert_eq!(body, "Eres el experto.");
        assert!(frontmatter("sin frontmatter").is_none());
    }

    #[test]
    fn store_dedupes_by_url_and_caps_table() {
        let l = |url: &str| Listing { store: "x".into(), title: "t".into(), price_pen: 1.0, url: url.into(),
            seller: None, rating: None, sold: None };
        let store = Store::default();
        assert_eq!(store.add(vec![l("a"), l("b"), l("a")]), [0, 1, 0]);
        let ids = store.add((0..25).map(|i| l(&i.to_string())).collect());
        assert!(store.table(&ids).ends_with("(5 resultados más omitidos; afina la búsqueda)"));
    }
}
