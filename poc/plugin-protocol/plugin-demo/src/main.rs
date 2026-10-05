//! semoxide-plugin-demo: a process plugin written against the raw protocol (no SDK).
//! `--mode <m>` switches on misbehaviour for tests:
//!   ok | crash | invalid | garbage | slow | badproto | stderr | noisy-child

use std::io::{BufRead, Write};

use serde_json::{Value, json};

fn send(v: Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{v}").unwrap();
    out.flush().unwrap();
}

fn log(level: &str, message: &str) {
    send(json!({"jsonrpc": "2.0", "method": "log", "params": {"level": level, "message": message}}));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.iter().position(|a| a == "--mode").and_then(|i| args.get(i + 1)).map(String::as_str).unwrap_or("ok");

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let method = msg["method"].as_str().unwrap_or("");
        let Some(id) = msg.get("id").cloned() else {
            if method == "shutdown" {
                return;
            }
            continue; // other notifications
        };
        let ctx = &msg["params"]["context"];
        let result: Result<Value, (i64, String, Option<&str>)> = match method {
            "initialize" => {
                let mut steps = vec!["verifyConditions", "generateNotes", "publish", "futureStep"];
                if mode == "invalid" {
                    steps.push("analyzeCommits");
                }
                let protocol = if mode == "badproto" { 99 } else { 1 };
                if mode == "garbage" {
                    println!("npm notice this line is not JSON-RPC");
                    println!("{{\"some\": \"json but not rpc\"}}");
                }
                Ok(json!({"protocol": protocol, "name": "demo", "steps": steps}))
            }
            "verifyConditions" => {
                if mode == "stderr" {
                    eprintln!("warning from demo on stderr, token={}", std::env::var("GH_TOKEN").unwrap_or_default());
                }
                match std::env::var("GH_TOKEN") {
                    // Deliberately logs the secret: the host must mask it.
                    Ok(t) => {
                        log("info", &format!("verified GH_TOKEN={t}"));
                        Ok(Value::Null)
                    }
                    Err(_) => Err((-32000, "GH_TOKEN is not set".into(), Some("ENOGHTOKEN"))),
                }
            }
            "analyzeCommits" if mode == "invalid" => Ok(json!("huge")),
            "generateNotes" => {
                let v = ctx["nextRelease"]["version"].as_str().unwrap_or("?");
                Ok(json!(format!("### Demo plugin\n- built {v}")))
            }
            "publish" => match mode {
                "crash" => std::process::exit(3),
                "slow" => {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    Ok(Value::Null)
                }
                "noisy-child" => {
                    // A child process inheriting our stdout pollutes the protocol stream.
                    let _ = std::process::Command::new("cmd").args(["/C", "echo", "child output on stdout"]).status();
                    Ok(json!({"name": "demo registry", "url": "https://example.invalid/demo"}))
                }
                _ => Ok(json!({"name": "demo registry", "url": format!("https://example.invalid/demo/{}", ctx["nextRelease"]["version"].as_str().unwrap_or("?"))})),
            },
            "bench" => Ok(Value::Null),
            other => Err((-32601, format!("method not found: {other}"), None)),
        };
        match result {
            Ok(r) => send(json!({"jsonrpc": "2.0", "id": id, "result": r})),
            Err((code, message, rc)) => {
                let data = rc.map(|c| json!({"code": c}));
                send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message, "data": data}}))
            }
        }
    }
}
