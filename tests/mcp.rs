use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn store_expert_speaks_mcp() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_obsession"))
        .args(["mcp", "amazon"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    drop(stdin);
    let replies: Vec<String> = stdout.lines().map(Result::unwrap).collect();
    child.wait().unwrap();
    assert_eq!(replies.len(), 2, "the notification must get no reply");
    assert!(replies[0].contains(r#""protocolVersion""#));
    assert!(replies[1].contains(r#""name":"search""#));
}
