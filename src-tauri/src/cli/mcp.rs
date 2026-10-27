use super::registry::{self, COMMANDS};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub const DEFAULT_PROTOCOL: &str = "2026-07-28";
const CAREFUL: &str = " It changes data or reaches other people; confirm with the user before calling it.";
const INSTRUCTIONS: &str = "Chatnip is the user's own unified inbox for Outlook, Teams, Gmail, Slack, IMAP and calendars on this Mac. Reach for these tools to read, search or summarise the user's mail and chats, look up people, work with cases, tags, drafts, reminders and signatures, or send a reply. Tools are named <resource>_<verb>; get reads, create adds, update changes, delete removes. Anything that sends or deletes asks the user first.";

pub fn tools() -> Vec<Value> {
    COMMANDS
        .iter()
        .map(|spec| {
            let mut description = format!("{}. Example: {}", registry::summary(spec), registry::example(spec));
            if !registry::reads_only(spec) {
                description.push_str(CAREFUL);
            }
            let mut tool = json!({
                "name": registry::tool_name(spec),
                "title": registry::summary(spec),
                "description": description,
                "inputSchema": registry::input_schema(spec),
                "outputSchema": { "type": "object" },
                "annotations": {
                    "readOnlyHint": registry::reads_only(spec),
                    "destructiveHint": registry::destructive(spec),
                    "idempotentHint": registry::idempotent(spec),
                    "openWorldHint": registry::reaches_people(spec),
                },
            });
            if registry::destructive(spec) || registry::reaches_people(spec) {
                tool["_meta"] = json!({ "anthropic/requiresUserInteraction": true });
            }
            tool
        })
        .collect()
}

fn text(body: String, error: bool) -> Value {
    let mut result = json!({ "content": [{ "type": "text", "text": if body.is_empty() { "(no output)".to_string() } else { body } }] });
    if error {
        result["isError"] = json!(true);
    }
    result
}

fn structured(body: String) -> Value {
    let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::String(body.clone()));
    let content = match &parsed {
        Value::Object(_) => parsed.clone(),
        other => json!({ "result": other }),
    };
    let mut result = text(body, false);
    result["structuredContent"] = content;
    result
}

fn call(params: &Value, report: &dyn Fn(u64, u64, &str)) -> Value {
    let name = params["name"].as_str().unwrap_or_default();
    let Some(spec) = registry::find_tool(name) else {
        return text(format!("no tool is called {}; tools/list names them", name), true);
    };
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let exe = match super::plugin::executable() {
        Ok(exe) => exe,
        Err(e) => return text(e, true),
    };
    let child = std::process::Command::new(exe)
        .args(["call", spec.name, "--raw", "--data", &arguments.to_string()])
        .env("NO_COLOR", "1")
        .env("CHATNIP_MCP", "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => return text(format!("chatnip could not start: {}", e), true),
    };
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut body = String::new();
        if let Some(mut stdout) = stdout {
            let _ = std::io::Read::read_to_string(&mut stdout, &mut body);
        }
        body
    });
    let mut errors: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
            match line.strip_prefix("step ").and_then(|s| serde_json::from_str::<Value>(s).ok()) {
                Some(step) => report(step["progress"].as_u64().unwrap_or(0), step["total"].as_u64().unwrap_or(0), step["message"].as_str().unwrap_or_default()),
                None => errors.push(line),
            }
        }
    }
    let body = reader.join().unwrap_or_default();
    match child.wait() {
        Ok(status) if status.success() => structured(body.trim().to_string()),
        Ok(_) => text(errors.join("\n").trim().trim_start_matches('✗').trim().to_string(), true),
        Err(e) => text(format!("chatnip did not finish: {}", e), true),
    }
}

pub fn progress(request: &Value, step: u64, total: u64, message: &str) -> Option<Value> {
    let token = request["params"]["_meta"]["progressToken"].clone();
    if token.is_null() || request["method"].as_str() != Some("tools/call") {
        return None;
    }
    Some(json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": { "progressToken": token, "progress": step, "total": total, "message": message } }))
}

pub fn answer(request: &Value, report: &dyn Fn(u64, u64, &str)) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request["method"].as_str().unwrap_or_default();
    let body = match method {
        "initialize" => {
            let protocol = request["params"]["protocolVersion"].as_str().unwrap_or(DEFAULT_PROTOCOL);
            json!({ "result": { "protocolVersion": protocol, "capabilities": { "tools": {} }, "serverInfo": { "name": super::plugin::NAME, "version": env!("CARGO_PKG_VERSION") }, "instructions": INSTRUCTIONS } })
        }
        "ping" => json!({ "result": {} }),
        "tools/list" => json!({ "result": { "tools": tools() } }),
        "tools/call" => json!({ "result": call(&request["params"], report) }),
        other => json!({ "error": { "code": -32601, "message": format!("method not found: {}", other) } }),
    };
    let mut body = body;
    body["jsonrpc"] = json!("2.0");
    body["id"] = id;
    Some(body)
}

pub fn serve(out: std::fs::File) -> i32 {
    let out = Arc::new(Mutex::new(out));
    let mut workers = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                let reply = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } });
                let _ = writeln!(out.lock().expect("mcp out"), "{}", reply);
                continue;
            }
        };
        let out = out.clone();
        workers.push(std::thread::spawn(move || {
            let tool = request["params"]["name"].as_str().unwrap_or_default().replace('_', " ");
            if let Some(started) = progress(&request, 0, 1, &format!("running {}", tool)) {
                let mut out = out.lock().expect("mcp out");
                let _ = writeln!(out, "{}", started);
                let _ = out.flush();
            }
            let notify = |step: u64, total: u64, message: &str| {
                if let Some(note) = progress(&request, step, total.max(1), message) {
                    let mut out = out.lock().expect("mcp out");
                    let _ = writeln!(out, "{}", note);
                    let _ = out.flush();
                }
            };
            let reply = answer(&request, &notify);
            let mut out = out.lock().expect("mcp out");
            if let Some(reply) = reply {
                let _ = writeln!(out, "{}", reply);
            }
            let _ = out.flush();
        }));
    }
    for worker in workers {
        let _ = worker.join();
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_the_protocol_names_the_server_and_says_when_to_reach_for_it() {
        let reply = answer(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }), &|_, _, _| {}).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(reply["result"]["serverInfo"]["name"], "chatnip");
        assert!(reply["result"]["instructions"].as_str().unwrap().contains("unified inbox"));
        let fallback = answer(&json!({ "id": 2, "method": "initialize" }), &|_, _, _| {}).unwrap();
        assert_eq!(fallback["result"]["protocolVersion"], DEFAULT_PROTOCOL);
    }

    #[test]
    fn every_command_is_a_tool_named_by_its_path_with_annotations() {
        let tools = tools();
        assert_eq!(tools.len(), COMMANDS.len());
        let conversations = tools.iter().find(|t| t["name"] == "conversations_get").unwrap();
        assert_eq!(conversations["inputSchema"]["type"], "object");
        assert_eq!(conversations["annotations"]["readOnlyHint"], true);
        assert!(!conversations["description"].as_str().unwrap().contains("confirm"));
        let send = tools.iter().find(|t| t["name"] == "message_create").unwrap();
        assert!(send["description"].as_str().unwrap().contains("confirm with the user"));
        assert_eq!(send["_meta"]["anthropic/requiresUserInteraction"], true);
        let delete = tools.iter().find(|t| t["name"] == "draft_delete").unwrap();
        assert_eq!(delete["annotations"]["destructiveHint"], true);
        assert!(tools.iter().all(|t| t["name"].as_str().unwrap().len() < 128 && t["name"].as_str().unwrap().chars().all(|c| c.is_ascii_alphanumeric() || c == '_')));
        let list = answer(&json!({ "id": 3, "method": "tools/list" }), &|_, _, _| {}).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), COMMANDS.len());
    }

    #[test]
    fn results_carry_structured_content_and_progress_follows_the_token() {
        assert_eq!(structured("[1,2]".into())["structuredContent"], json!({ "result": [1, 2] }));
        assert_eq!(structured("{\"a\":1}".into())["structuredContent"], json!({ "a": 1 }));
        let call = json!({ "id": 9, "method": "tools/call", "params": { "name": "conversations_get", "_meta": { "progressToken": "p1" } } });
        assert_eq!(progress(&call, 0, 1, "running").unwrap()["params"]["progressToken"], "p1");
        assert!(progress(&json!({ "id": 9, "method": "tools/call", "params": {} }), 0, 1, "x").is_none());
    }

    #[test]
    fn notifications_get_no_answer_and_unknown_methods_an_error() {
        assert!(answer(&json!({ "method": "notifications/initialized" }), &|_, _, _| {}).is_none());
        let reply = answer(&json!({ "id": 4, "method": "resources/list" }), &|_, _, _| {}).unwrap();
        assert_eq!(reply["error"]["code"], -32601);
        let unknown = answer(&json!({ "id": 5, "method": "tools/call", "params": { "name": "nope" } }), &|_, _, _| {}).unwrap();
        assert_eq!(unknown["result"]["isError"], true);
    }
}
