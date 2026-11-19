//! Minimal MCP over stdio (newline-delimited JSON-RPC 2.0): a server that exposes one store
//! as a `search` tool, and a client the orchestrator uses to call any expert in the catalog.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PROTOCOL: &str = "2025-06-18";

/// `obsession mcp <store>`: serve one store expert until stdin closes.
pub fn serve(store: &str) -> Result<(), String> {
    if !crate::stores::STORES.contains(&store) {
        return Err(format!("tienda desconocida: {store}"));
    }
    let client = crate::stores::client();
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = req.get("id").cloned() else { continue }; // notifications need no reply
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": format!("obsession-{store}"), "version": env!("CARGO_PKG_VERSION")}
            })),
            "tools/list" => Ok(json!({"tools": [{
                "name": "search",
                "description": format!("Busca productos en {store} (solo lectura). Devuelve listings JSON con precio en soles."),
                "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
            }]})),
            "tools/call" if req["params"]["name"] == "search" => {
                let query = req["params"]["arguments"]["query"].as_str().unwrap_or("");
                Ok(match crate::stores::search(&client, store, query) {
                    Ok(l) => json!({"content": [{"type": "text", "text": serde_json::to_string(&l).unwrap()}]}),
                    Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
                })
            }
            m => Err(json!({"code": -32601, "message": format!("método no soportado: {m}")})),
        };
        let reply = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        let mut out = stdout.lock();
        writeln!(out, "{reply}").and_then(|_| out.flush()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    pub fn spawn(command: &str, args: &[String]) -> Result<Self, String> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{command}: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut c = Client { child, stdin, stdout, next_id: 0 };
        c.request("initialize", json!({
            "protocolVersion": PROTOCOL, "capabilities": {},
            "clientInfo": {"name": "obsession", "version": env!("CARGO_PKG_VERSION")}
        }))?;
        c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        Ok(c)
    }

    /// Calls a tool and returns its text content; a tool error comes back as `Err`.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<String, String> {
        let r = self.request("tools/call", json!({"name": tool, "arguments": arguments}))?;
        let text: String = r["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .collect();
        if r["isError"] == true { Err(text) } else { Ok(text) }
    }

    fn send(&mut self, msg: &Value) -> Result<(), String> {
        writeln!(self.stdin, "{msg}").and_then(|_| self.stdin.flush()).map_err(|e| e.to_string())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err(format!("el servidor MCP cerró durante {method}"));
            }
            let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
            if msg["id"] != id {
                continue; // server notifications or logs
            }
            if let Some(err) = msg.get("error") {
                return Err(format!("{method}: {}", err["message"]));
            }
            return Ok(msg["result"].clone());
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
