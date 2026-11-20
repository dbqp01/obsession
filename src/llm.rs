use crate::providers;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Tokens one run has spent, shared by its parallel experts, and the cap it must stay under.
pub struct Usage {
    prompt: AtomicU64,
    completion: AtomicU64,
    max: u64,
}

impl Usage {
    pub fn new(max: u64) -> Self {
        Usage { prompt: AtomicU64::new(0), completion: AtomicU64::new(0), max }
    }

    /// `OBSESSION_MAX_TOKENS`, 400000 by default.
    pub fn from_env() -> Self {
        Self::new(std::env::var("OBSESSION_MAX_TOKENS").ok().and_then(|v| v.parse().ok()).unwrap_or(400_000))
    }

    /// Adds the `usage` block of one chat completion response.
    pub fn add(&self, resp: &Value) {
        self.prompt.fetch_add(resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0), Ordering::Relaxed);
        self.completion.fetch_add(resp["usage"]["completion_tokens"].as_u64().unwrap_or(0), Ordering::Relaxed);
    }

    pub fn total(&self) -> u64 {
        self.prompt.load(Ordering::Relaxed) + self.completion.load(Ordering::Relaxed)
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn spent(&self) -> bool {
        self.total() >= self.max
    }

    pub fn json(&self) -> Value {
        json!({"prompt": self.prompt.load(Ordering::Relaxed), "completion": self.completion.load(Ordering::Relaxed),
            "total": self.total()})
    }
}

/// POSTs one OpenAI-compatible chat completion to the model's provider (magpie unless `byok:`),
/// counts its tokens and parses it, with one retry.
fn post<T>(model: &str, mut body: Value, usage: &Usage, parse: impl Fn(&Value) -> Result<T, String>) -> Result<T, String> {
    let route = providers::route(model)?;
    body["model"] = json!(route.model);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(420))
        .build()
        .map_err(|e| e.to_string())?;
    let mut last_err = String::new();
    // One retry: cheap models sometimes time out, or wrap or truncate the JSON.
    for _ in 0..2 {
        let mut req = client.post(format!("{}/chat/completions", route.url)).json(&body);
        if let Some(key) = &route.key {
            req = req.bearer_auth(key);
        }
        match req.send().and_then(|r| r.error_for_status()).and_then(|r| r.json::<Value>()) {
            Ok(resp) => {
                usage.add(&resp);
                match parse(&resp) {
                    Ok(v) => return Ok(v),
                    Err(e) => last_err = e,
                }
            }
            Err(e) => last_err = format!("{model}: {e}"),
        }
    }
    Err(last_err)
}

/// One chat call that must answer JSON; returns the JSON the model wrote.
pub fn chat_json(model: &str, system: &str, user: &str, usage: &Usage) -> Result<Value, String> {
    let body = json!({
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
        "max_tokens": 8000,
    });
    post(model, body, usage, |resp| {
        let content = resp["choices"][0]["message"]["content"].as_str().unwrap_or("");
        extract_json(content)
            .ok_or_else(|| format!("{model} no devolvió JSON: {}", &content.chars().take(200).collect::<String>()))
    })
}

/// One agent step: sends the conversation with tools and returns the assistant message.
/// `force` makes the model call a tool instead of answering in prose.
pub fn chat_tools(model: &str, messages: &[Value], tools: &[Value], force: bool, usage: &Usage) -> Result<Value, String> {
    let body = json!({
        "messages": messages, "tools": tools, "max_tokens": 4000,
        "tool_choice": if force { "required" } else { "auto" },
    });
    post(model, body, usage, |resp| Ok(resp["choices"][0]["message"].clone()))
}

/// Takes the outermost JSON object or array out of a reply that may carry fences or prose.
pub fn extract_json(text: &str) -> Option<Value> {
    let start = text.find(['{', '['])?;
    let close = if text[start..].starts_with('{') { '}' } else { ']' };
    let end = text.rfind(close)?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

#[cfg(test)]
mod tests {
    use super::{Usage, extract_json};
    use serde_json::json;

    #[test]
    fn extracts_fenced_and_prose_wrapped_json() {
        assert_eq!(extract_json("```json\n{\"a\":1}\n```").unwrap()["a"], 1);
        assert_eq!(extract_json("Claro: [1,2] listo").unwrap()[1], 2);
        assert!(extract_json("sin json").is_none());
    }

    #[test]
    fn accumulates_usage_and_checks_budget() {
        let usage = Usage::new(1000);
        let resp = json!({"choices": [], "usage": {"prompt_tokens": 600, "completion_tokens": 150, "total_tokens": 750}});
        usage.add(&resp);
        assert!(!usage.spent());
        usage.add(&json!({"choices": []}));
        assert_eq!(usage.total(), 750, "a response without usage adds nothing");
        usage.add(&resp);
        assert!(usage.spent());
        assert_eq!(usage.json(), json!({"prompt": 1200, "completion": 300, "total": 1500}));
    }
}
