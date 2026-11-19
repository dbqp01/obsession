use serde_json::{Value, json};
use std::time::Duration;

/// OpenAI-compatible chat call through the magpie router; returns the JSON the model wrote.
pub fn chat_json(model: &str, system: &str, user: &str) -> Result<Value, String> {
    let base = std::env::var("MAGPIE_URL").unwrap_or_else(|_| "http://127.0.0.1:3425/v1".into());
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(420))
        .build()
        .map_err(|e| e.to_string())?;
    let body = json!({
        "model": model,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
        "max_tokens": 8000,
    });
    let mut last_err = String::new();
    // One retry: cheap models sometimes time out, or wrap or truncate the JSON.
    for _ in 0..2 {
        let resp: Value = match client
            .post(format!("{base}/chat/completions"))
            .json(&body)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
        {
            Ok(v) => v,
            Err(e) => {
                last_err = format!("{model}: {e}");
                continue;
            }
        };
        let content = resp["choices"][0]["message"]["content"].as_str().unwrap_or("");
        match extract_json(content) {
            Some(v) => return Ok(v),
            None => last_err = format!("{model} no devolvió JSON: {}", &content.chars().take(200).collect::<String>()),
        }
    }
    Err(last_err)
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
    use super::extract_json;

    #[test]
    fn extracts_fenced_and_prose_wrapped_json() {
        assert_eq!(extract_json("```json\n{\"a\":1}\n```").unwrap()["a"], 1);
        assert_eq!(extract_json("Claro: [1,2] listo").unwrap()[1], 2);
        assert!(extract_json("sin json").is_none());
    }
}
