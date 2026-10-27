use crate::models::action::AiChoice;
use crate::models::app_setting::{self, AppSetting};
use crate::models::intelligence::{IntelligenceConnection, PutSectionSummary, SectionSummary, KIND_CLAUDE_CLI, KIND_COMMAND, KIND_OLLAMA, KIND_OPENAI_COMPATIBLE, KIND_REPLICATE, SECTION_KIND_ALL, SECTION_KIND_SELECTION, SECTION_KIND_SECTION};
use crate::services::action_script::{self, Diagnostic};
use crate::models::tag::{ObjectTag, Tag, KIND_SPLIT};
use crate::models::{Person, Profile};
use async_trait::async_trait;
use chrono::{Local, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{Pool, Row, Sqlite};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct PromptRun {
    pub key: String,
    pub connection: String,
    pub model: Option<String>,
    pub rendered_prompt: String,
    pub output: Value,
    pub cost_usd: f64,
    pub duration_ms: u64,
}

pub struct Answer {
    pub output: Value,
    pub cost_usd: f64,
}

#[async_trait]
pub trait Origin: Send + Sync {
    async fn available(&self) -> (bool, Option<String>);
    async fn models(&self) -> Result<Vec<String>, String>;
    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String>;
}

fn parse_structured(text: &str, schema: Option<&Value>) -> Value {
    let trimmed = text.trim();
    if schema.is_none() {
        return Value::String(trimmed.to_string());
    }
    if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
        return v;
    }
    let stripped = trimmed.trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    if let Ok(v) = serde_json::from_str::<Value>(stripped) {
        return v;
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(v) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
            return v;
        }
    }
    Value::Null
}

pub struct ClaudeCli {
    pub binary: String,
    pub model: String,
}

#[async_trait]
impl Origin for ClaudeCli {
    async fn available(&self) -> (bool, Option<String>) {
        match resolve_binary(&self.binary) {
            Some(path) => (true, Some(path.to_string_lossy().to_string())),
            None => (false, Some(format!("{} was not found on this machine", self.binary))),
        }
    }

    async fn models(&self) -> Result<Vec<String>, String> {
        let mut models: Vec<String> = if self.model.is_empty() { Vec::new() } else { vec![self.model.clone()] };
        for model in claude_status(false).await.models {
            if !models.contains(&model) {
                models.push(model);
            }
        }
        Ok(models)
    }

    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String> {
        let model = model.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(&self.model);
        let binary = resolve_binary(&self.binary).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| self.binary.clone());
        let mut args: Vec<String> = vec!["-p".into(), "--output-format".into(), "json".into()];
        if let Some(schema) = schema {
            args.push("--json-schema".into());
            args.push(schema.to_string());
        }
        args.push("--no-session-persistence".into());
        if !model.is_empty() {
            args.extend(["--model".into(), model.to_string()]);
        }
        args.extend(["--system-prompt".into(), system.to_string(), "--tools".into(), String::new()]);
        let mut child = Command::new(&binary)
            .args(&args)
            .env("HOME", crate::cli::plugin::home())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Could not start {}: {}. Is Claude Code installed and on the path?", binary, e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await.map_err(|e| format!("Could not hand the prompt to Claude: {}", e))?;
            drop(stdin);
        }
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| "Claude took too long and was stopped".to_string())?
            .map_err(|e| format!("Claude failed to run: {}", e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let envelope: Value = serde_json::from_str(stdout.trim()).map_err(|_| {
            let detail = if stderr.trim().is_empty() { stdout.trim().to_string() } else { stderr.trim().to_string() };
            format!("Claude answered with something unreadable: {}", detail.chars().take(300).collect::<String>())
        })?;
        if envelope["is_error"].as_bool() == Some(true) {
            return Err(format!("Claude refused: {}", envelope["result"].as_str().unwrap_or("unknown error")));
        }
        let output = match &envelope["structured_output"] {
            Value::Null => {
                let text = envelope["result"].as_str().unwrap_or("");
                parse_structured(text, schema)
            }
            other => other.clone(),
        };
        if output.is_null() {
            return Err("Claude answered without the structure it was asked for".to_string());
        }
        Ok(Answer { output, cost_usd: envelope["total_cost_usd"].as_f64().unwrap_or(0.0) })
    }
}

pub struct Replicate {
    pub api_token: String,
    pub model: String,
}

impl Replicate {
    fn client() -> reqwest::Client {
        reqwest::Client::new()
    }

    fn split_model(model: &str) -> Result<(String, String, Option<String>), String> {
        let (path, version) = match model.split_once(':') {
            Some((p, v)) => (p, Some(v.to_string())),
            None => (model, None),
        };
        let (owner, name) = path.split_once('/').ok_or_else(|| "A Replicate model is written owner/name".to_string())?;
        Ok((owner.to_string(), name.to_string(), version))
    }
}

#[async_trait]
impl Origin for Replicate {
    async fn available(&self) -> (bool, Option<String>) {
        if self.api_token.is_empty() {
            return (false, Some("No Replicate API token".to_string()));
        }
        (true, Some(format!("model {}", if self.model.is_empty() { "not chosen" } else { &self.model })))
    }

    async fn models(&self) -> Result<Vec<String>, String> {
        let mut seen = BTreeSet::new();
        let client = Self::client();
        if let Ok(resp) = client.get("https://api.replicate.com/v1/predictions").bearer_auth(&self.api_token).send().await {
            if let Ok(body) = resp.json::<Value>().await {
                for item in body["results"].as_array().cloned().unwrap_or_default() {
                    if let Some(m) = item["model"].as_str() {
                        seen.insert(m.to_string());
                    }
                }
            }
        }
        let resp = client
            .get("https://api.replicate.com/v1/models")
            .query(&[("query", "llm")])
            .bearer_auth(&self.api_token)
            .send()
            .await
            .map_err(|e| format!("Replicate did not answer: {}", e))?;
        let body: Value = resp.json().await.map_err(|e| format!("Replicate answered unreadably: {}", e))?;
        for item in body["results"].as_array().cloned().unwrap_or_default().iter().take(20) {
            if let (Some(owner), Some(name)) = (item["owner"].as_str(), item["name"].as_str()) {
                seen.insert(format!("{}/{}", owner, name));
            }
        }
        if !self.model.is_empty() {
            seen.insert(self.model.clone());
        }
        Ok(seen.into_iter().collect())
    }

    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String> {
        let model = model.filter(|m| !m.is_empty()).unwrap_or(&self.model);
        if model.is_empty() {
            return Err(action_script::REQUIREMENT_AI_MODEL.error());
        }
        let (owner, name, version) = Self::split_model(model)?;
        let prompt_text = match schema {
            Some(schema) => format!("{}\n\nAnswer with JSON only, matching this schema:\n{}", prompt, schema),
            None => prompt.to_string(),
        };
        let body = json!({ "input": { "prompt": prompt_text, "system_prompt": system } });
        let client = Self::client();
        let request = match &version {
            Some(v) => client.post("https://api.replicate.com/v1/predictions").json(&json!({ "version": v, "input": body["input"] })),
            None => client.post(format!("https://api.replicate.com/v1/models/{}/{}/predictions", owner, name)).json(&body),
        };
        let started = client
            .execute(request.bearer_auth(&self.api_token).header("Prefer", "wait").build().map_err(|e| e.to_string())?)
            .await
            .map_err(|e| format!("Replicate did not answer: {}", e))?;
        let mut prediction: Value = started.json().await.map_err(|e| format!("Replicate answered unreadably: {}", e))?;
        if let Some(detail) = prediction["detail"].as_str() {
            return Err(format!("Replicate refused: {}", detail));
        }
        let deadline = tokio::time::Instant::now() + timeout;
        while matches!(prediction["status"].as_str(), Some("starting") | Some("processing")) {
            if tokio::time::Instant::now() > deadline {
                return Err("Replicate took too long".to_string());
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let url = prediction["urls"]["get"].as_str().ok_or_else(|| "Replicate gave no polling url".to_string())?.to_string();
            prediction = client
                .get(&url)
                .bearer_auth(&self.api_token)
                .send()
                .await
                .map_err(|e| format!("Replicate did not answer: {}", e))?
                .json()
                .await
                .map_err(|e| format!("Replicate answered unreadably: {}", e))?;
        }
        if prediction["status"].as_str() != Some("succeeded") {
            return Err(format!("Replicate ended with {}: {}", prediction["status"].as_str().unwrap_or("unknown"), prediction["error"].as_str().unwrap_or("")));
        }
        let text = match &prediction["output"] {
            Value::Array(parts) => parts.iter().filter_map(|p| p.as_str()).collect::<Vec<_>>().join(""),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let output = parse_structured(&text, schema);
        if output.is_null() {
            return Err("Replicate answered without the structure it was asked for".to_string());
        }
        let cost = prediction["metrics"]["predict_time"].as_f64().unwrap_or(0.0);
        Ok(Answer { output, cost_usd: cost * 0.0 })
    }
}

pub struct OpenAiCompatible {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl OpenAiCompatible {
    pub fn request_body(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>) -> Value {
        let model = model.filter(|m| !m.is_empty()).unwrap_or(&self.model);
        let mut body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": prompt }
            ]
        });
        if let Some(schema) = schema {
            body["response_format"] = json!({ "type": "json_schema", "json_schema": { "name": "answer", "schema": schema } });
        }
        body
    }

    pub fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let versioned = base.rsplit('/').next().map(|last| last.len() > 1 && last.starts_with('v') && last[1..].chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
        let path = path.trim_start_matches('/');
        if versioned { format!("{}/{}", base, path) } else { format!("{}/v1/{}", base, path) }
    }
}

#[async_trait]
impl Origin for OpenAiCompatible {
    async fn available(&self) -> (bool, Option<String>) {
        if self.base_url.is_empty() {
            return (false, Some("No base url".to_string()));
        }
        (true, Some(self.base_url.clone()))
    }

    async fn models(&self) -> Result<Vec<String>, String> {
        let mut req = reqwest::Client::new().get(self.url("models"));
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let body: Value = req
            .send()
            .await
            .map_err(|e| format!("{} did not answer: {}", self.base_url, e))?
            .json()
            .await
            .map_err(|e| format!("{} answered unreadably: {}", self.base_url, e))?;
        let mut models: Vec<String> = body["data"].as_array().cloned().unwrap_or_default().iter().filter_map(|m| m["id"].as_str().map(String::from)).collect();
        if !self.model.is_empty() && !models.contains(&self.model) {
            models.insert(0, self.model.clone());
        }
        Ok(models)
    }

    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String> {
        let body = self.request_body(system, prompt, model, schema);
        if body["model"].as_str().unwrap_or("").is_empty() {
            return Err(action_script::REQUIREMENT_AI_MODEL.error());
        }
        let mut req = reqwest::Client::new().post(self.url("chat/completions")).timeout(timeout).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let answer: Value = req
            .send()
            .await
            .map_err(|e| format!("{} did not answer: {}", self.base_url, e))?
            .json()
            .await
            .map_err(|e| format!("{} answered unreadably: {}", self.base_url, e))?;
        if let Some(message) = answer["error"]["message"].as_str() {
            return Err(format!("{} refused: {}", self.base_url, message));
        }
        let text = answer["choices"][0]["message"]["content"].as_str().unwrap_or("");
        let output = parse_structured(text, schema);
        if output.is_null() {
            return Err("The model answered without the structure it was asked for".to_string());
        }
        Ok(Answer { output, cost_usd: 0.0 })
    }
}

pub const OLLAMA_DOWNLOAD: &str = "https://ollama.com/download";

pub struct Ollama {
    pub host: String,
    pub model: String,
}

impl Ollama {
    fn chat(&self) -> OpenAiCompatible {
        OpenAiCompatible { base_url: self.host.clone(), api_key: String::new(), model: self.model.clone() }
    }
}

#[async_trait]
impl Origin for Ollama {
    async fn available(&self) -> (bool, Option<String>) {
        match ollama_version(&self.host).await {
            Some(version) => (true, Some(format!("Ollama {} at {}", version, self.host))),
            None => (false, Some(format!("Ollama is not running at {}", self.host))),
        }
    }

    async fn models(&self) -> Result<Vec<String>, String> {
        Ok(ollama_models(&self.host).await?.into_iter().map(|m| m.name).collect())
    }

    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String> {
        self.chat().run(system, prompt, model, schema, timeout).await
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OllamaModel {
    pub name: String,
    pub size_bytes: u64,
    pub parameter_size: Option<String>,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OllamaStatus {
    pub installed: bool,
    pub binary: Option<String>,
    pub running: bool,
    pub version: Option<String>,
    pub host: String,
    pub models: Vec<OllamaModel>,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelSuggestion {
    pub name: String,
    pub size: String,
    pub good_for: String,
    pub command: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaudeStatus {
    pub installed: bool,
    pub binary: Option<String>,
    pub version: Option<String>,
    pub signed_in: bool,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub model: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PullProgress {
    pub model: String,
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub done: bool,
    pub error: Option<String>,
}

pub fn suggestions(status: &OllamaStatus) -> Vec<ModelSuggestion> {
    if !status.running {
        return Vec::new();
    }
    let have = |name: &str| status.models.iter().any(|m| m.name == name || m.name == format!("{}:latest", name));
    app_setting::OLLAMA_SUGGESTIONS
        .iter()
        .filter(|s| !have(s.name))
        .map(|s| ModelSuggestion { name: s.name.to_string(), size: s.size.to_string(), good_for: s.good_for.to_string(), command: format!("ollama pull {}", s.name) })
        .collect()
}

pub fn parse_tags(body: &Value) -> Vec<OllamaModel> {
    body["models"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|m| {
                    let name = m["name"].as_str().or_else(|| m["model"].as_str())?.to_string();
                    Some(OllamaModel {
                        name,
                        size_bytes: m["size"].as_u64().unwrap_or(0),
                        parameter_size: m["details"]["parameter_size"].as_str().map(String::from),
                        family: m["details"]["family"].as_str().map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_pull_line(model: &str, line: &str) -> Option<PullProgress> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let error = value["error"].as_str().map(String::from);
    let status = value["status"].as_str().unwrap_or(if error.is_some() { "error" } else { "" }).to_string();
    Some(PullProgress { model: model.to_string(), done: status == "success" || error.is_some(), status, completed: value["completed"].as_u64(), total: value["total"].as_u64(), error })
}

fn probe() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_millis(1500)).build().unwrap_or_default()
}

pub async fn ollama_version(host: &str) -> Option<String> {
    let body: Value = probe().get(format!("{}/api/version", host.trim_end_matches('/'))).send().await.ok()?.json().await.ok()?;
    body["version"].as_str().map(String::from)
}

pub async fn ollama_models(host: &str) -> Result<Vec<OllamaModel>, String> {
    let body: Value = probe()
        .get(format!("{}/api/tags", host.trim_end_matches('/')))
        .send()
        .await
        .map_err(|_| format!("Ollama is not running at {}", host))?
        .json()
        .await
        .map_err(|e| format!("Ollama answered unreadably: {}", e))?;
    Ok(parse_tags(&body))
}

static OLLAMA_STATUS: OnceLock<tokio::sync::Mutex<HashMap<String, OllamaStatus>>> = OnceLock::new();

pub async fn ollama_status(host: &str, refresh: bool) -> OllamaStatus {
    let mut cached = OLLAMA_STATUS.get_or_init(Default::default).lock().await;
    if let (false, Some(status)) = (refresh, cached.get(host)) {
        return status.clone();
    }
    let binary = resolve_binary("ollama").map(|p| p.to_string_lossy().to_string());
    let version = ollama_version(host).await;
    let models = if version.is_some() { ollama_models(host).await.unwrap_or_default() } else { Vec::new() };
    let status = OllamaStatus { installed: binary.is_some() || version.is_some() || ollama_app().is_some(), binary, running: version.is_some(), version, host: host.to_string(), models, download_url: OLLAMA_DOWNLOAD.to_string() };
    cached.insert(host.to_string(), status.clone());
    status
}

fn ollama_app() -> Option<PathBuf> {
    [PathBuf::from("/Applications"), crate::cli::plugin::home().join("Applications")].into_iter().map(|dir| dir.join("Ollama.app")).find(|app| app.is_dir())
}

pub async fn start_ollama(host: &str) -> Result<OllamaStatus, String> {
    if ollama_version(host).await.is_none() {
        if ollama_app().is_some() {
            let opened = Command::new("open").args(["-a", "Ollama"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output().await.map_err(|e| format!("Could not open the Ollama app: {}", e))?;
            if !opened.status.success() {
                return Err(format!("The Ollama app did not open: {}", String::from_utf8_lossy(&opened.stderr).trim()));
            }
        } else {
            let binary = resolve_binary("ollama").ok_or_else(|| "Ollama is not installed on this machine".to_string())?;
            let mut child = Command::new(&binary)
                .arg("serve")
                .env("HOME", crate::cli::plugin::home())
                .env("OLLAMA_HOST", host)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Could not start {}: {}", binary.display(), e))?;
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
        let mut waited = 0;
        while ollama_version(host).await.is_none() {
            if waited >= 20 {
                return Err(format!("Ollama did not answer at {} after starting", host));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            waited += 1;
        }
    }
    Ok(ollama_status(host, true).await)
}

pub async fn pull_ollama_model(host: &str, model: &str, mut emit: impl FnMut(PullProgress) + Send) -> Result<(), String> {
    if !model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_' | '/')) || model.is_empty() {
        return Err(format!("{} is not a model name", model));
    }
    let mut response = reqwest::Client::new()
        .post(format!("{}/api/pull", host.trim_end_matches('/')))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|_| format!("Ollama is not running at {}. Start it, then pull again.", host))?;
    let mut buffer = String::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("The pull of {} stopped: {}", model, e))? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buffer.find('\n') {
            let line: String = buffer.drain(..=end).collect();
            if let Some(progress) = parse_pull_line(model, &line) {
                let failed = progress.error.clone();
                emit(progress);
                if let Some(error) = failed {
                    return Err(error);
                }
            }
        }
    }
    if let Some(progress) = parse_pull_line(model, &buffer) {
        emit(progress);
    }
    ollama_status(host, true).await;
    Ok(())
}

fn text_of(value: &Value) -> Option<String> {
    value.as_str().map(str::trim).filter(|v| !v.is_empty()).map(String::from)
}

pub fn claude_account(auth: &Value) -> Option<String> {
    if auth["loggedIn"].as_bool() != Some(true) {
        return None;
    }
    text_of(&auth["email"]).or_else(|| text_of(&auth["orgName"]))
}

pub fn claude_models(help: &str, settings: &Value) -> (Option<String>, Vec<String>) {
    let model = text_of(&settings["model"]).or_else(|| text_of(&settings["env"]["ANTHROPIC_MODEL"]));
    let allowed: Vec<String> = settings["availableModels"].as_array().map(|items| items.iter().filter_map(text_of).collect()).unwrap_or_default();
    let listed = if !allowed.is_empty() {
        allowed
    } else {
        static QUOTED: OnceLock<Regex> = OnceLock::new();
        let option = help.find("--model <").map(|start| &help[start..]).unwrap_or_default();
        let option = option.get(1..).and_then(|rest| rest.find("\n  -")).map(|end| &option[..end + 1]).unwrap_or(option);
        crate::content::rx(&QUOTED, r"'([A-Za-z0-9][\w.:\[\]-]*)'").captures_iter(option).map(|c| c[1].to_string()).collect()
    };
    let mut models: Vec<String> = Vec::new();
    for entry in model.iter().cloned().chain(listed) {
        if !models.contains(&entry) {
            models.push(entry);
        }
    }
    (model, models)
}

async fn claude_output(binary: &std::path::Path, args: &[&str]) -> Option<String> {
    let output = tokio::time::timeout(Duration::from_secs(10), Command::new(binary).args(args).env("HOME", crate::cli::plugin::home()).stdin(Stdio::null()).kill_on_drop(true).output()).await.ok()?.ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string()).filter(|v| !v.is_empty())
}

static CLAUDE_STATUS: OnceLock<tokio::sync::Mutex<Option<ClaudeStatus>>> = OnceLock::new();

pub async fn claude_status(refresh: bool) -> ClaudeStatus {
    let mut cached = CLAUDE_STATUS.get_or_init(Default::default).lock().await;
    if let (false, Some(status)) = (refresh, cached.as_ref()) {
        return status.clone();
    }
    let status = match detect_claude() {
        Some(path) => {
            let (version, help, auth) = tokio::join!(claude_output(&path, &["--version"]), claude_output(&path, &["--help"]), claude_output(&path, &["auth", "status", "--json"]));
            let auth: Value = auth.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null);
            let config = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| crate::cli::plugin::home().join(".claude"));
            let settings: Value = std::fs::read_to_string(config.join("settings.json")).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or(Value::Null);
            let (model, models) = claude_models(help.as_deref().unwrap_or_default(), &settings);
            ClaudeStatus { installed: true, binary: Some(path.to_string_lossy().to_string()), version, signed_in: auth["loggedIn"].as_bool() == Some(true), account: claude_account(&auth), plan: text_of(&auth["subscriptionType"]), model, models }
        }
        None => ClaudeStatus::default(),
    };
    *cached = Some(status.clone());
    status
}

pub struct CommandOrigin {
    pub command: String,
    pub args: String,
    pub stdin: String,
    pub output: String,
    pub model_arg: String,
    pub models_command: String,
    pub model: String,
}

impl CommandOrigin {
    pub fn argv(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>) -> Result<Vec<String>, String> {
        let model = model.filter(|m| !m.is_empty()).unwrap_or(&self.model);
        let context = json!({ "system": system, "prompt": prompt, "model": model, "schema": schema.map(|s| s.to_string()).unwrap_or_default() });
        let rendered = render(&self.args, &context)?;
        let mut argv = split_args(&rendered);
        if !self.model_arg.is_empty() && !model.is_empty() {
            argv.push(self.model_arg.clone());
            argv.push(model.to_string());
        }
        Ok(argv)
    }
}

pub fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match (ch, quote) {
            ('\\', Some('\'')) => current.push(ch),
            ('\\', _) => escaped = true,
            ('"', None) | ('\'', None) => quote = Some(ch),
            (c, Some(q)) if c == q => quote = None,
            (c, None) if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            (c, _) => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[async_trait]
impl Origin for CommandOrigin {
    async fn available(&self) -> (bool, Option<String>) {
        if self.command.is_empty() {
            return (false, Some("No command".to_string()));
        }
        match resolve_binary(&self.command) {
            Some(path) => (true, Some(path.to_string_lossy().to_string())),
            None => (false, Some(format!("{} was not found on this machine", self.command))),
        }
    }

    async fn models(&self) -> Result<Vec<String>, String> {
        if self.models_command.trim().is_empty() {
            return Ok(if self.model.is_empty() { Vec::new() } else { vec![self.model.clone()] });
        }
        let argv = split_args(&self.models_command);
        let (program, rest) = argv.split_first().ok_or_else(|| "The models command is empty".to_string())?;
        let output = tokio::time::timeout(Duration::from_secs(30), Command::new(program).args(rest).output())
            .await
            .map_err(|_| "The models command took too long".to_string())?
            .map_err(|e| format!("The models command failed: {}", e))?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
    }

    async fn run(&self, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>, timeout: Duration) -> Result<Answer, String> {
        if self.command.is_empty() {
            return Err("The command origin has no command".to_string());
        }
        let argv = self.argv(system, prompt, model, schema)?;
        let feed_stdin = self.stdin != "none";
        let mut child = Command::new(&self.command)
            .args(&argv)
            .stdin(if feed_stdin { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Could not start {}: {}", self.command, e))?;
        if feed_stdin {
            if let Some(mut stdin) = child.stdin.take() {
                let text = if self.stdin == "full" { format!("{}\n\n{}", system, prompt) } else { prompt.to_string() };
                stdin.write_all(text.as_bytes()).await.map_err(|e| format!("Could not hand the prompt to {}: {}", self.command, e))?;
                drop(stdin);
            }
        }
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| format!("{} took too long and was stopped", self.command))?
            .map_err(|e| format!("{} failed to run: {}", self.command, e))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() && stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(format!("{} exited with {}: {}", self.command, output.status, stderr.trim().chars().take(300).collect::<String>()));
        }
        let parsed = if self.output == "text" {
            if schema.is_some() { parse_structured(&stdout, schema) } else { Value::String(stdout.trim().to_string()) }
        } else {
            match parse_structured(&stdout, Some(&Value::Null)) {
                Value::Null => Value::String(stdout.trim().to_string()),
                v => v,
            }
        };
        if parsed.is_null() {
            return Err(format!("{} answered without the structure it was asked for", self.command));
        }
        Ok(Answer { output: parsed, cost_usd: 0.0 })
    }
}

pub fn resolve_binary(name: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(name);
    if candidate.components().count() > 1 {
        return candidate.is_file().then_some(candidate);
    }
    for dir in search_dirs() {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".claude/local"));
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".npm-global/bin"));
        dirs.push(home.join("Library/pnpm"));
        dirs.push(home.join(".cargo/bin"));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

pub fn detect_claude() -> Option<PathBuf> {
    resolve_binary("claude")
}

pub fn origin_for(connection: &IntelligenceConnection) -> Result<Box<dyn Origin>, String> {
    let get = |key: &str| connection.setting(key).unwrap_or_default();
    match connection.kind.as_str() {
        KIND_CLAUDE_CLI => Ok(Box::new(ClaudeCli {
            binary: connection.setting("binary").unwrap_or_else(|| app_setting::get_default(app_setting::ASSISTANT_BINARY).unwrap_or_default().to_string()),
            model: get("model"),
        })),
        KIND_REPLICATE => Ok(Box::new(Replicate { api_token: get("api_token"), model: get("model") })),
        KIND_OPENAI_COMPATIBLE => Ok(Box::new(OpenAiCompatible { base_url: get("base_url"), api_key: get("api_key"), model: get("model") })),
        KIND_OLLAMA => Ok(Box::new(Ollama { host: connection.setting("host").filter(|h| !h.is_empty()).unwrap_or_else(|| app_setting::get_default(app_setting::OLLAMA_HOST).unwrap_or_default().to_string()), model: get("model") })),
        KIND_COMMAND => Ok(Box::new(CommandOrigin {
            command: get("command"),
            args: get("args"),
            stdin: connection.setting("stdin").unwrap_or_else(|| "prompt".to_string()),
            output: connection.setting("output").unwrap_or_else(|| "json".to_string()),
            model_arg: get("model_arg"),
            models_command: get("models_command"),
            model: get("model"),
        })),
        other => Err(format!("Unknown intelligence kind {}", other)),
    }
}

pub async fn describe(pool: &Pool<Sqlite>, mut connection: IntelligenceConnection) -> IntelligenceConnection {
    if connection.kind == KIND_CLAUDE_CLI {
        if connection.setting("binary").is_none() {
            if let Ok(binary) = AppSetting::get_value(pool, app_setting::ASSISTANT_BINARY).await {
                connection.config.insert("binary".to_string(), binary);
            }
        }
        if connection.setting("model").is_none() {
            if let Ok(model) = AppSetting::get_value(pool, app_setting::ASSISTANT_MODEL).await.map(|m| m.trim().to_string()) {
                if !model.is_empty() {
                    connection.config.insert("model".to_string(), model);
                }
            }
        }
    }
    if connection.kind == KIND_OLLAMA && connection.setting("host").is_none() {
        if let Ok(host) = AppSetting::get_value(pool, app_setting::OLLAMA_HOST).await {
            connection.config.insert("host".to_string(), host);
        }
    }
    if connection.kind == KIND_OLLAMA {
        let host = connection.setting("host").unwrap_or_default();
        let status = ollama_status(&host, false).await;
        connection.available = status.running;
        connection.detail = Some(match &status.version {
            Some(version) => format!("Ollama {} at {}", version, host),
            None => format!("Ollama is not running at {}", host),
        });
        connection.models = status.models.into_iter().map(|m| m.name).collect();
        return connection;
    }
    match origin_for(&connection) {
        Ok(origin) => {
            let (available, detail) = origin.available().await;
            connection.available = available;
            connection.detail = detail;
            if connection.kind == KIND_CLAUDE_CLI {
                connection.models = origin.models().await.unwrap_or_default();
            } else if let Some(model) = connection.setting("model") {
                connection.models = vec![model];
            }
        }
        Err(e) => {
            connection.available = false;
            connection.detail = Some(e);
        }
    }
    connection
}

pub async fn ensure_defaults(pool: &Pool<Sqlite>, profile_id: &str) -> Result<(), String> {
    if IntelligenceConnection::all(pool, profile_id).await?.is_empty() {
        if let Some(path) = detect_claude() {
            let mut config = std::collections::BTreeMap::new();
            config.insert("binary".to_string(), path.to_string_lossy().to_string());
            IntelligenceConnection::post(
                pool,
                profile_id,
                crate::models::intelligence::PostIntelligenceConnectionInput { kind: KIND_CLAUDE_CLI.to_string(), name: "Claude Code".to_string(), config, is_default: Some(true) },
            )
            .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Prompt {
    pub key: String,
    pub name: String,
    pub description: String,
    pub connection_id: Option<String>,
    pub model: Option<String>,
    pub system: String,
    pub template: String,
    pub output_schema: Option<String>,
    pub variables: Vec<TemplateVariable>,
}

pub fn prompt(key: &str, overrides: &Value) -> Result<Prompt, String> {
    let seed = seeds().into_iter().find(|s| s.key == key).ok_or_else(|| format!("There is no prompt called {}", key))?;
    let field = |name: &str| overrides.get(name).and_then(Value::as_str).map(|v| v.to_string());
    let filled = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
    let system = field("system").unwrap_or_else(|| seed.system.to_string());
    let template = filled(field("template")).unwrap_or_else(|| seed.template.to_string());
    Ok(Prompt {
        key: seed.key.to_string(),
        name: seed.name.to_string(),
        description: seed.description.to_string(),
        connection_id: filled(field("connection_id")),
        model: filled(field("model")),
        variables: variables(&format!("{}\n{}", system, template)),
        output_schema: filled(field("output_schema").or(seed.schema.map(String::from))),
        system,
        template,
    })
}

pub fn prompts() -> Vec<Prompt> {
    seeds().iter().filter_map(|s| prompt(s.key, &Value::Null).ok()).collect()
}

pub fn overrides(key: &str, system: &str, template: &str, output_schema: Option<&str>, connection_id: Option<&str>, model: Option<&str>) -> Option<Value> {
    let seed = seeds().into_iter().find(|s| s.key == key)?;
    let mut changed = Map::new();
    if system != seed.system {
        changed.insert("system".into(), json!(system));
    }
    if template != seed.template {
        changed.insert("template".into(), json!(template));
    }
    if output_schema != seed.schema {
        changed.insert("output_schema".into(), json!(output_schema.unwrap_or_default()));
    }
    for (name, value) in [("connection_id", connection_id), ("model", model)] {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            changed.insert(name.into(), json!(value));
        }
    }
    (!changed.is_empty()).then_some(Value::Object(changed))
}

pub struct Seed {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub system: &'static str,
    pub template: &'static str,
    pub schema: Option<&'static str>,
}

const SUBJECT_TEMPLATE: &str = "Source: {{ .subject.provider_type }}\nConnection: {{ .subject.connection_name }}\nKind: {{ .subject.conversation_type }}\nConversation: {{ .subject.conversation_name }}\nLatest sender: {{ .subject.sender_name }} <{{ .subject.sender_email }}>\nSubject: {{ .subject.subject }}\nMembers: {{ .subject.member_count }}\n\nLatest message:\n{{ .subject.body }}";

pub fn seeds() -> Vec<Seed> {
    vec![
        Seed {
            key: "sort",
            name: "Sort into a split",
            description: "Runs on every new conversation no rule caught. Picks the inbox split it belongs to.",
            system: "You sort one conversation from {{ .profile.name }}'s unified inbox into exactly one split. The splits and what belongs in them:\n{{ range .splits }}- {{ .name }}: {{ .description }}\n{{ end }}Answer with the split name, a confidence between 0 and 1, and one short reason. Favour Important when a person writes to {{ .profile.name }} directly or asks something of them.",
            template: SUBJECT_TEMPLATE,
            schema: Some(r#"{"type":"object","properties":{"label":{"type":"string","enum":{{ .split_names_json }}},"confidence":{"type":"number","minimum":0,"maximum":1},"reason":{"type":"string"}},"required":["label","confidence","reason"]}"#),
        },
        Seed {
            key: "parse_events",
            name: "Find appointments",
            description: "Reads one message and lists the meetings, calls and deadlines it proposes, for the calendar.",
            system: "You read one message from {{ .profile.name }}'s inbox and list every concrete appointment, meeting, deadline or call it proposes or confirms, with a start time. Resolve relative dates against the message date. Use the timezone offset {{ .message.tz_offset }} unless the message states another. Skip past events the message only mentions, newsletters, and vague intentions. Give a confidence between 0 and 1. An empty list is a fine answer.",
            template: "Message date: {{ .message.date }}\nConversation: {{ .message.conversation_name }}\nFrom: {{ .message.from_name }} <{{ .message.from_email }}>\nSubject: {{ .message.subject }}\n\n{{ .message.text }}",
            schema: Some(r#"{"type":"object","properties":{"events":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"starts_at":{"type":"string","description":"RFC 3339 with the offset of the sender's timezone"},"ends_at":{"type":"string"},"all_day":{"type":"boolean"},"location":{"type":"string"},"attendees":{"type":"array","items":{"type":"string"}},"confidence":{"type":"number","minimum":0,"maximum":1},"notes":{"type":"string"}},"required":["title","starts_at","confidence"]}}},"required":["events"]}"#),
        },
        Seed {
            key: "summarise_thread",
            name: "Summarise a thread",
            description: "Five lines on what a conversation is about and what is waiting on you.",
            system: "You summarise part of a conversation from {{ .profile.name }}'s unified inbox. Write {{ if .style }}{{ .style.length }}, {{ .style.form }}, {{ .style.language }}{{ else }}at most five short lines, in the language the conversation uses{{ end }}. Lead with what matters most to {{ .profile.name }}: decisions made, what is asked of them, deadlines and open questions. Resolve relative dates such as Thursday or tomorrow against the message time and today ({{ .today }}), and say when something has already passed. Mention attached files by name when they carry the substance. Skip greetings, signatures and quoted earlier mail. Never invent facts that are not in the messages.{{ if .style }}{{ if .style.instructions }} {{ .style.instructions }}{{ end }}{{ end }}{{ if .focus }} Focus on: {{ .focus }}.{{ end }} {{ if .style }}{{ if .style.actions }}List the concrete action items for {{ .profile.name }} separately.{{ else }}Leave action_items empty.{{ end }}{{ else }}List the concrete action items separately.{{ end }}",
            template: "Conversation: {{ .conversation.name }} ({{ .conversation.type }} on {{ .conversation.provider }})\nMembers: {{ range .conversation.members }}{{ . }}, {{ end }}\n\n{{ range .messages }}[{{ .time }}] {{ .sender }}: {{ .content }}{{ if .files }} [files: {{ range .files }}{{ . }}; {{ end }}]{{ end }}\n{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"summary":{"type":"string"},"action_items":{"type":"array","items":{"type":"string"}}},"required":["summary","action_items"]}"#),
        },
        Seed {
            key: "summarise_mail",
            name: "Summarise one email",
            description: "One short line on what a single email says, asks or decides. Shown on folded mails in a thread.",
            system: "Write the gist of one email to {{ .profile.name }} as a single line of at most 80 characters, in the same language as the email body: a Norwegian email gets a Norwegian line. State the point itself as a terse headline, the way a busy reader would jot it down, for example \"Godtar 40 timer, overføring maks ett kvartal\" or \"Asks to move the review to Thursday 13:00\". Never describe the email (no \"summarised\", \"the sender\", \"this email\", \"reply\"), never name the sender, never repeat the subject, no greeting, no trailing full stop.",
            template: "From: {{ .message.from_name }} <{{ .message.from_email }}>\nSubject: {{ .message.subject }}\n\n{{ .message.text }}",
            schema: Some(r#"{"type":"object","properties":{"line":{"type":"string","maxLength":120,"description":"The headline itself, written in the language of the email, for example: Godtar 40 timer, overføring maks ett kvartal"}},"required":["line"]}"#),
        },
        Seed {
            key: "script_ai",
            name: "Answer for an action",
            description: "Used by ai() in action scripts. Answers the prompt the action gives, in one plain answer.",
            system: "You help an automation in {{ .profile.name }}'s unified inbox. Answer the request directly and briefly, in the language of the material you are given. Never invent facts that are not in the material.",
            template: "{{ .prompt }}{{ if .material }}\n\nMaterial:\n{{ .material }}{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"]}"#),
        },
        Seed {
            key: "script_ai_json",
            name: "Structured answer for an action",
            description: "Used by ai_json(), classify() and summarize() in action scripts. The action supplies the answer's JSON schema.",
            system: "You help an automation in {{ .profile.name }}'s unified inbox. Answer with the structure asked for and nothing else. Base every field on the material; leave a field empty rather than guess.",
            template: "{{ .prompt }}{{ if .material }}\n\nMaterial:\n{{ .material }}{{ end }}",
            schema: Some("{{ .schema_json }}"),
        },
        Seed {
            key: "author_action",
            name: "Manage actions",
            description: "The Actions assistant. Answers each chat message with operations on your actions in the node format: create, update, copy, split, merge, delete, move and rename.",
            system: "You manage the actions of Chatnip, the unified inbox on {{ .profile.name }}'s Mac, by proposing operations on them in its node format. Answer ONLY with one JSON object, {\"reply\": \"<one short sentence>\", \"operations\": [ ... ]}, and nothing else: no prose around it, no code fences, no comments. Keep operations empty when you only answer a question or need to ask one, and ask in the reply.\n\nAn action is JSON: {\"name\", \"description\", \"icon\" (an Iconify name such as lucide:zap), \"max_sends_per_hour\", \"inputs\": [{\"name\", \"type\", \"description\"}], \"graph\": {\"nodes\": [{\"id\", \"type\", \"label\", \"config\", \"body\", \"x\", \"y\"}], \"edges\": [{\"id\", \"source\", \"source_port\", \"target\", \"target_port\"}]}}. A node id is a short unique snake_case word. An edge id is source.source_port-target.target_port. Lay nodes out left to right, 300 apart in x, rows 180 apart in y.\n\nWiring rules. An edge joins an output port the source node has to an input port the target node has. The types must fit: equal, either side any, the input json, or the input text and the output tag, number or bool. Nothing wires into a trigger. A landing ends the flow and nothing continues from it. A chat node with something wired in ends the flow. No loops, no node wired to itself, and an action never contains itself, directly or through another action. An input left unwired takes its constant from config.values. An action starts at a trigger, or at a chat node with nothing wired in. What an action gives back is the outputs of its final nodes, the ones whose outputs wire nowhere; config.returns renames one as {\"<port>\": {\"name\", \"description\"}}. An action meant to run inside others starts at a trigger with config.on called, whose outputs carry the action inputs.\n\nFrames and nesting. A node of type action:<id> runs that whole action as one step and shows as a frame holding its graph; its ports are the action's inputs and outputs, config.values fills its inputs, and config.collapsed folds the frame.\n\nResults. Every run, dry or real, is kept under Runs with a result line: the outputs of the final nodes, else the effects it listed. Give every action at least one text output named message that says in one sentence what it did, so a run never reads as no outputs. Every run also ends with a result object, kind command, content, data or none, message, commands, content and outputs, which the place the action is attached to reads as commands to apply or content to show.\n\nControls. A trigger with config.on schedule carries config.schedule { every_minutes, at as HH:MM, days as 1 to 7 }, and with event_soon config.minutes_before; these show as controls on the node itself and in the inspector, so fill them with sensible values rather than null. A trigger may carry config.labels, { \"<port>\": \"<what it is here>\" }, such as { \"person\": \"who left\" }, shown on the node beside the port; the port names stay as they are. Call only the functions in the catalogue: the editor marks any other call as an error before a run. The dry run dialog shows one tab per action input, such as message, conversation and person, with fields from the input schema, and runs the action on that mock; nothing is sent. A present_mail node with a template replaces how its mails render, so a scenario such as receipts or newsletters gets its own view; the template receives message.mail, the sanitised body.\n\nThe node catalogue, read from this Mac just now:\n{{ .catalogue }}\n\nOperations, applied in order:\n- {\"op\": \"create\", \"ref\": an optional name later operations use as its action_id, \"group_id\": optional, \"action\": the complete action}\n- {\"op\": \"update\", \"action_id\", \"action\": only the fields that change; inputs and graph are always given whole}\n- {\"op\": \"copy\", \"action_id\", \"name\": optional, \"without_trigger\": optional, \"ref\": optional}\n- {\"op\": \"split\", \"action_id\", \"node_ids\": the nodes that move into a new action, \"name\", \"ref\": optional}; the original keeps one action node in their place, wired the same way\n- {\"op\": \"merge\", \"action_ids\": two or more, \"name\", \"remove_sources\": optional, \"ref\": optional}\n- {\"op\": \"delete\", \"action_id\"}\n- {\"op\": \"move\", \"action_ids\", \"group_id\": a group id, or null for the top level}\n- {\"op\": \"rename\", \"action_id\", \"name\"}\nBuilt in actions and built in groups are read only; copy one to change it. Use only the node types, ports, functions, tags, groups and action ids listed here. When a selected action is given, the user means that one unless they say otherwise. The user reviews every operation before it is applied.",
            template: "{{ if .scope }}Selected action:\n{{ .scope }}\n\n{{ end }}Actions:\n{{ .actions }}\n{{ if .detail }}\nYour actions in full:\n{{ .detail }}\n{{ end }}\nAction groups:\n{{ .groups }}\n{{ if .messages }}\nThe conversation so far:\n{{ range .messages }}{{ .role }}: {{ .content }}{{ if .status }} [{{ .status }}]{{ end }}\n{{ end }}{{ end }}\nuser: {{ .instruction }}{{ if .errors }}\n\nYour previous answer was refused. Fix every point and answer again:\n{{ range .errors }}- {{ . }}\n{{ end }}Previous answer:\n{{ .previous }}{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"reply":{"type":"string"},"operations":{"type":"array","items":{"type":"object","properties":{"op":{"type":"string","enum":["create","update","copy","split","merge","delete","move","rename"]},"ref":{"type":"string"},"action_id":{"type":"string"},"action_ids":{"type":"array","items":{"type":"string"}},"node_ids":{"type":"array","items":{"type":"string"}},"group_id":{"type":["string","null"]},"name":{"type":"string"},"without_trigger":{"type":"boolean"},"remove_sources":{"type":"boolean"},"action":{"type":"object"}},"required":["op"]}}},"required":["reply","operations"]}"#),
        },
        Seed {
            key: "author_template",
            name: "Write a message template",
            description: "The template assistant in Actions. Writes or edits the Svelte 5 component that renders a message for a presentation node.",
            system: "You write Svelte 5 components that render one message in Chatnip, a unified inbox on {{ .profile.name }}'s Mac. Use runes only: let { message, presentation, invite, theme } = $props(), $derived and $state. message has id, type, subject, text, html, mail, sender, sender_email and at; mail is the mail body as safe html with inline images resolved and remote images blocked, or null when the message is not html, and {@html message.mail} shows it. presentation has kind, badge, highlight, pinned, collapsed, summary_only and icon. invite has title, starts_at, ends_at, location, organizer and join_url, or is null. Never import anything, never fetch, never use window.parent, localStorage or timers that run forever. Style with a scoped style block using the CSS variables --paper, --paper-2, --ink, --ink-2, --ink-3, --line, --accent and --font-sans; sizes are relative. {{ if .guidance }}The node asks for this: {{ .guidance }} {{ end }}Return the complete component source and one sentence on what changed.",
            template: "Kind: {{ .kind }}\nInstruction: {{ .instruction }}\n\nCurrent source:\n{{ .source }}",
            schema: Some(r#"{"type":"object","properties":{"source":{"type":"string"},"explanation":{"type":"string"}},"required":["source","explanation"]}"#),
        },
        Seed {
            key: "classify_sender",
            name: "Human or service",
            description: "Decides whether a sender is a person or an automated service, as a second opinion on the pattern rules.",
            system: "Decide whether the sender below is a human being writing personally or an automated service, notification, newsletter, bot or shared mailbox. Answer human or service with one short reason.",
            template: "Name: {{ .sender.name }}\nEmail: {{ .sender.email }}\nSource: {{ .sender.provider_type }}\n{{ if .sender.sample }}Latest message:\n{{ .sender.sample }}{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"kind":{"type":"string","enum":["human","service"]},"reason":{"type":"string"}},"required":["kind","reason"]}"#),
        },
        Seed {
            key: "reply_draft",
            name: "Draft a reply",
            description: "Writes a reply in your tone from the thread and an instruction, ready to edit in the composer.",
            system: "You draft a reply for {{ .profile.name }} in the language and tone the thread uses, short and direct, never signing it, never inventing facts. Follow the instruction when one is given.",
            template: "Conversation: {{ .conversation.name }} ({{ .conversation.type }} on {{ .conversation.provider }})\n{{ if .instruction }}Instruction: {{ .instruction }}\n{{ end }}\n{{ range .messages }}[{{ .time }}] {{ .sender }}: {{ .content }}\n{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"reply":{"type":"string"}},"required":["reply"]}"#),
        },
        Seed {
            key: "receipt_confirm",
            name: "Confirm receipts",
            description: "Runs in the Receipts action. A small local model reads the candidates the patterns found and says, one by one, whether each is a receipt for something paid.",
            system: "You check whether texts are receipts: proof that something was bought and paid for. Order confirmations, paid invoices, tickets and payment confirmations count. Unpaid invoices, offers, newsletters, shipping notices and messages that only confirm receipt of an application do not. Answer with JSON only, one verdict for every candidate.",
            template: "Give each candidate its id, receipt true or false, a confidence between 0 and 1, and for a receipt the vendor, the total as a number, the currency code and the purchase date as YYYY-MM-DD.\n\n{{ range .candidates }}Candidate {{ .id }}\nFrom: {{ .sender }}\nSubject: {{ .subject }}\n{{ .text }}\n\n{{ end }}",
            schema: Some(r#"{"type":"object","properties":{"verdicts":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"receipt":{"type":"boolean"},"confidence":{"type":"number","minimum":0,"maximum":1},"vendor":{"type":"string"},"total":{"type":"number"},"currency":{"type":"string"},"date":{"type":"string"}},"required":["id","receipt","confidence"]}}},"required":["verdicts"]}"#),
        },
        Seed {
            key: crate::services::voice_service::PROMPT_KEY,
            name: "Clean up dictation",
            description: "Turns what voice heard into written text while you dictate and when you stop: punctuation, casing, numbers and lists, never other words.",
            system: crate::services::voice_service::PROMPT_SYSTEM,
            template: crate::services::voice_service::PROMPT_TEMPLATE,
            schema: None,
        },
    ]
}

fn to_gtmpl(value: &Value) -> gtmpl::Value {
    match value {
        Value::Null => gtmpl::Value::Nil,
        Value::Bool(b) => gtmpl::Value::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                gtmpl::Value::Number(i.into())
            } else {
                gtmpl::Value::Number(n.as_f64().unwrap_or(0.0).into())
            }
        }
        Value::String(s) => gtmpl::Value::String(s.clone()),
        Value::Array(items) => gtmpl::Value::Array(items.iter().map(to_gtmpl).collect()),
        Value::Object(map) => gtmpl::Value::Map(map.iter().map(|(k, v)| (k.clone(), to_gtmpl(v))).collect::<HashMap<String, gtmpl::Value>>()),
    }
}

fn gtmpl_text(value: Option<&gtmpl::Value>) -> String {
    match value {
        Some(gtmpl::Value::String(s)) => s.clone(),
        Some(gtmpl::Value::Nil) | Some(gtmpl::Value::NoValue) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn gtmpl_empty(value: Option<&gtmpl::Value>) -> bool {
    match value {
        None | Some(gtmpl::Value::Nil) | Some(gtmpl::Value::NoValue) => true,
        Some(gtmpl::Value::String(s)) => s.trim().is_empty(),
        Some(gtmpl::Value::Bool(b)) => !b,
        Some(gtmpl::Value::Array(items)) => items.is_empty(),
        Some(gtmpl::Value::Map(map)) => map.is_empty(),
        Some(_) => false,
    }
}

fn helper_lower(args: &[gtmpl::Value]) -> Result<gtmpl::Value, gtmpl::FuncError> {
    Ok(gtmpl::Value::String(gtmpl_text(args.last()).to_lowercase()))
}

fn helper_upper(args: &[gtmpl::Value]) -> Result<gtmpl::Value, gtmpl::FuncError> {
    Ok(gtmpl::Value::String(gtmpl_text(args.last()).to_uppercase()))
}

fn helper_default(args: &[gtmpl::Value]) -> Result<gtmpl::Value, gtmpl::FuncError> {
    match args {
        [] => Err(gtmpl::FuncError::AtLeastXArgs("default".into(), 1)),
        [fallback] => Ok(fallback.clone()),
        [fallback, .., value] => Ok(if gtmpl_empty(Some(value)) { fallback.clone() } else { value.clone() }),
    }
}

const GO_LAYOUT: [(&str, &str); 22] = [
    ("January", "%B"),
    ("Monday", "%A"),
    ("-07:00", "%:z"),
    ("-0700", "%z"),
    ("2006", "%Y"),
    ("Jan", "%b"),
    ("Mon", "%a"),
    ("MST", "%Z"),
    ("_2", "%e"),
    ("01", "%m"),
    ("02", "%d"),
    ("03", "%I"),
    ("04", "%M"),
    ("05", "%S"),
    ("06", "%y"),
    ("15", "%H"),
    ("PM", "%p"),
    ("pm", "%P"),
    ("1", "%-m"),
    ("2", "%-d"),
    ("3", "%-I"),
    ("4", "%-M"),
];

pub fn go_layout(layout: &str) -> String {
    if layout.contains('%') {
        return layout.to_string();
    }
    let mut out = String::new();
    let mut rest = layout;
    'outer: while !rest.is_empty() {
        for (token, spec) in GO_LAYOUT {
            if let Some(after) = rest.strip_prefix(token) {
                out.push_str(spec);
                rest = after;
                continue 'outer;
            }
        }
        let ch = rest.chars().next().unwrap_or_default();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    out
}

pub fn parse_when(text: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let text = text.trim();
    if text.is_empty() || text == "now" {
        return Some(Local::now().fixed_offset());
    }
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(at);
    }
    let local = |naive: chrono::NaiveDateTime| naive.and_local_timezone(Local).single().map(|at| at.fixed_offset());
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%A %Y-%m-%d %H:%M"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(text, format) {
            return local(naive);
        }
    }
    for format in ["%Y-%m-%d", "%A %Y-%m-%d"] {
        if let Ok(day) = chrono::NaiveDate::parse_from_str(text, format) {
            return local(day.and_hms_opt(0, 0, 0)?);
        }
    }
    None
}

fn helper_date(args: &[gtmpl::Value]) -> Result<gtmpl::Value, gtmpl::FuncError> {
    let layout = gtmpl_text(args.first());
    let value = if args.len() > 1 { gtmpl_text(args.last()) } else { String::new() };
    let spec = go_layout(if layout.is_empty() { "2006-01-02" } else { &layout });
    Ok(gtmpl::Value::String(match parse_when(&value) {
        Some(at) => {
            let mut out = String::new();
            use std::fmt::Write as _;
            if write!(out, "{}", at.format(&spec)).is_err() {
                return Err(gtmpl::FuncError::Generic(format!("date cannot use the layout {}", layout)));
            }
            out
        }
        None => value,
    }))
}

fn template(source: &str) -> Result<gtmpl::Template, String> {
    let mut tmpl = gtmpl::Template::default();
    tmpl.add_funcs(&[("lower", helper_lower as gtmpl::Func), ("upper", helper_upper as gtmpl::Func), ("default", helper_default as gtmpl::Func), ("date", helper_date as gtmpl::Func)]);
    tmpl.parse(source).map_err(|e| format!("The template does not parse: {}", e))?;
    Ok(tmpl)
}

pub fn render(source: &str, context: &Value) -> Result<String, String> {
    let tmpl = template(source)?;
    let mut filled = context.clone();
    fill_missing(&mut filled, &variables(source));
    tmpl.render(&gtmpl::Context::from(to_gtmpl(&filled))).map_err(|e| format!("The template could not be rendered: {}", e))
}

pub fn check_template(source: &str) -> Result<(), String> {
    template(source).map(|_| ())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TemplateVariable {
    pub name: String,
    pub kind: String,
    pub fields: Vec<String>,
    pub sample: String,
}

pub const VARIABLE_TEXT: &str = "text";
pub const VARIABLE_LIST: &str = "list";
pub const VARIABLE_FLAG: &str = "flag";

enum Scope {
    Range(String),
    With(String),
    Plain,
}

fn rank(kind: &str) -> u8 {
    match kind {
        VARIABLE_LIST => 3,
        VARIABLE_TEXT => 2,
        _ => 1,
    }
}

fn lookup<'a>(context: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(context, |value, key| value.get(key)).filter(|v| !v.is_null())
}

fn note(found: &mut BTreeMap<String, (String, Vec<String>)>, name: &str, kind: &str) {
    let entry = found.entry(name.to_string()).or_insert_with(|| (kind.to_string(), Vec::new()));
    if rank(kind) > rank(&entry.0) {
        entry.0 = kind.to_string();
    }
}

pub fn variables(source: &str) -> Vec<TemplateVariable> {
    static ACTION: OnceLock<Regex> = OnceLock::new();
    static FIELD: OnceLock<Regex> = OnceLock::new();
    static QUOTED: OnceLock<Regex> = OnceLock::new();
    let action = ACTION.get_or_init(|| Regex::new(r"(?s)\{\{-?(.*?)-?\}\}").expect("action regex"));
    let field = FIELD.get_or_init(|| Regex::new(r"(?:^|[^A-Za-z0-9_)\]$])(\$?)\.([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)").expect("field regex"));
    let quoted = QUOTED.get_or_init(|| Regex::new(r#""(?:[^"\\]|\\.)*"|`[^`]*`"#).expect("quoted regex"));
    let mut found: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    let mut scopes: Vec<Scope> = Vec::new();
    for cap in action.captures_iter(source) {
        let text = quoted.replace_all(cap[1].trim(), "\"\"").to_string();
        if text.starts_with("/*") {
            continue;
        }
        let keyword = text.split_whitespace().next().unwrap_or_default();
        let (expression, kind) = match keyword {
            "end" => {
                scopes.pop();
                continue;
            }
            "else" => (text.trim_start_matches("else").trim().trim_start_matches("if").to_string(), VARIABLE_FLAG),
            "if" => (text[2..].to_string(), VARIABLE_FLAG),
            "range" => (text[5..].to_string(), VARIABLE_LIST),
            "with" => (text[4..].to_string(), VARIABLE_FLAG),
            "define" | "block" | "template" => (String::new(), VARIABLE_TEXT),
            _ => (text.clone(), VARIABLE_TEXT),
        };
        let inner = scopes.iter().rev().find(|s| !matches!(s, Scope::Plain));
        let mut first: Option<String> = None;
        for f in field.captures_iter(&expression) {
            let path = f[2].to_string();
            let resolved = match (&f[1] == "$", inner) {
                (false, Some(Scope::Range(list))) => {
                    note(&mut found, list, VARIABLE_LIST);
                    if let Some(entry) = found.get_mut(list) {
                        if !entry.1.contains(&path) {
                            entry.1.push(path.clone());
                        }
                    }
                    if first.is_none() {
                        first = Some(format!("{}.{}", list, path));
                    }
                    continue;
                }
                (false, Some(Scope::With(base))) => format!("{}.{}", base, path),
                _ => path,
            };
            let this = if kind == VARIABLE_LIST && first.is_some() { VARIABLE_TEXT } else { kind };
            note(&mut found, &resolved, this);
            if first.is_none() {
                first = Some(resolved);
            }
        }
        match keyword {
            "range" => scopes.push(first.map(Scope::Range).unwrap_or(Scope::Plain)),
            "with" => scopes.push(first.map(Scope::With).unwrap_or(Scope::Plain)),
            "if" | "define" | "block" => scopes.push(Scope::Plain),
            _ => {}
        }
    }
    let samples = sample_context();
    found
        .into_iter()
        .map(|(name, (kind, mut fields))| {
            fields.sort();
            let sample = match lookup(&samples, &name) {
                Some(Value::String(s)) if kind != VARIABLE_LIST => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                _ => String::new(),
            };
            TemplateVariable { name, kind, fields, sample }
        })
        .collect()
}

pub fn missing_variables(source: &str, context: &Value) -> Vec<String> {
    variables(source).into_iter().filter(|v| lookup(context, &v.name).is_none()).map(|v| v.name).collect()
}

fn fill_missing(context: &mut Value, variables: &[TemplateVariable]) {
    let mut ordered: Vec<&TemplateVariable> = variables.iter().collect();
    ordered.sort_by_key(|v| v.name.matches('.').count());
    for variable in ordered {
        if lookup(context, &variable.name).is_some() {
            continue;
        }
        let parts: Vec<&str> = variable.name.split('.').collect();
        let mut node = &mut *context;
        for (index, part) in parts.iter().enumerate() {
            if !node.is_object() {
                *node = Value::Object(Map::new());
            }
            let Value::Object(map) = node else { break };
            if index + 1 == parts.len() {
                let blank = if variable.kind == VARIABLE_LIST { Value::Array(Vec::new()) } else { Value::String(String::new()) };
                map.insert(part.to_string(), blank);
                break;
            }
            node = map.entry(part.to_string()).or_insert(Value::Null);
        }
    }
}

pub fn sample_context() -> Value {
    let now = Local::now();
    json!({
        "profile": { "name": "Milo Oakes" },
        "me": { "name": "Milo Oakes", "first_name": "Milo", "email": "milo@northwind.example" },
        "person": { "id": "", "name": "Kari Nordmann", "first_name": "Kari", "email": "kari@acme.no", "is_human": true, "providers": ["outlook"] },
        "conversation": { "id": "", "name": "Faktura oktober", "type": "email", "provider": "outlook", "connection": "Northwind", "namespace": "", "summary": "Kari asks for the October invoice to be paid", "members": ["Kari Nordmann", "Milo Oakes"], "labels": ["Kunde"] },
        "message": { "id": "", "subject": "Faktura oktober", "from_name": "Kari Nordmann", "from_email": "kari@acme.no", "conversation_name": "Faktura oktober", "date": now.format("%A %Y-%m-%d %H:%M").to_string(), "tz_offset": now.format("%:z").to_string(), "text": "Hei Milo, kan du se på fakturaen for oktober?" },
        "messages": [
            { "sender": "Kari Nordmann", "email": "kari@acme.no", "time": now.format("%Y-%m-%d %H:%M").to_string(), "content": "Hei Milo, kan du se på fakturaen for oktober?" }
        ],
        "today": now.format("%A %Y-%m-%d").to_string(),
        "now": now.format("%Y-%m-%d %H:%M").to_string(),
        "instruction": "",
        "prompt": "",
        "material": ""
    })
}

async fn timeout_for(pool: &Pool<Sqlite>) -> Duration {
    Duration::from_secs(AppSetting::get_u64(pool, app_setting::INTELLIGENCE_TIMEOUT_SECONDS).await.unwrap_or_else(|_| app_setting::get_default_u64(app_setting::INTELLIGENCE_TIMEOUT_SECONDS)))
}

pub fn first_name(name: &str) -> String {
    name.split_whitespace().next().unwrap_or_default().to_string()
}

pub async fn base_context(pool: &Pool<Sqlite>, profile: &Profile, conversation_id: Option<&str>) -> Result<Value, String> {
    let records = Tag::all(pool, &profile.id).await?;
    let splits: Vec<Value> = records.iter().filter(|l| l.kind == KIND_SPLIT).map(|l| json!({ "name": l.name, "description": l.description, "color": l.color })).collect();
    let tags: Vec<Value> = records.iter().filter(|l| l.kind != KIND_SPLIT).map(|l| json!({ "name": l.name, "description": l.description, "color": l.color })).collect();
    let split_names: Vec<String> = records.iter().filter(|l| l.kind == KIND_SPLIT).map(|l| l.name.clone()).collect();
    let tag_names: Vec<String> = records.iter().map(|l| l.name.clone()).collect();
    let mut context = Map::new();
    context.insert("profile".into(), json!({ "name": profile.name }));
    context.insert("today".into(), Value::String(Local::now().format("%A %Y-%m-%d").to_string()));
    context.insert("now".into(), Value::String(Local::now().format("%Y-%m-%d %H:%M").to_string()));
    context.insert("labels".into(), Value::Array(records.iter().map(|l| json!({ "name": l.name, "kind": l.kind, "description": l.description })).collect()));
    context.insert("splits".into(), Value::Array(splits));
    context.insert("tags".into(), Value::Array(tags));
    context.insert("split_names".into(), json!(split_names));
    context.insert("split_names_json".into(), Value::String(serde_json::to_string(&split_names).unwrap_or_else(|_| "[]".into())));
    context.insert("label_names".into(), Value::String(tag_names.join(", ")));
    context.insert("instruction".into(), Value::String(String::new()));
    context.insert("person".into(), Value::Null);
    let my_email: Option<String> = match &profile.user_id {
        Some(user_id) => sqlx::query_scalar("SELECT email FROM user WHERE id = ?1").bind(user_id).fetch_optional(pool).await.map_err(|e| format!("Failed to load your account: {}", e))?.flatten(),
        None => None,
    };
    let account = match conversation_id {
        Some(id) => crate::models::Account::get_conversation(pool, id, None).await?,
        None => None,
    };
    let my_name = account.as_ref().and_then(|a| a.name()).unwrap_or_else(|| profile.name.clone());
    let my_email = account.and_then(|a| a.email).or(my_email);
    context.insert("me".into(), json!({ "name": my_name, "first_name": first_name(&my_name), "email": my_email.unwrap_or_default() }));
    if let Some(id) = conversation_id {
        let row = sqlx::query(
            "SELECT c.name, c.type, c.namespace, c.summary, c.person_id, p.provider_type, p.name AS connection_name
             FROM conversation c
             LEFT JOIN conversation_provider cp ON cp.conversation_id = c.id AND cp.is_primary = 1
             LEFT JOIN provider p ON p.id = cp.provider_id
             WHERE c.id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to load the conversation: {}", e))?;
        if let Some(row) = row {
            let members: Vec<String> = sqlx::query(
                "SELECT COALESCE(u.display_name, u.email, '') AS name FROM conversation_member cm JOIN user u ON u.id = cm.user_id WHERE cm.conversation_id = ?1",
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load members: {}", e))?
            .into_iter()
            .map(|r| r.get::<String, _>("name"))
            .filter(|n| !n.is_empty())
            .collect();
            let conversation_tags: Vec<String> = ObjectTag::get_conversation(pool, id)
                .await
                .unwrap_or_default()
                .into_iter()
                .filter_map(|cl| records.iter().find(|l| l.id == cl.tag_id).map(|l| l.name.clone()))
                .collect();
            let person_id: Option<String> = row.try_get("person_id").ok().flatten();
            context.insert(
                "conversation".into(),
                json!({
                    "id": id,
                    "name": row.get::<String, _>("name"),
                    "type": row.get::<String, _>("type"),
                    "namespace": row.try_get::<Option<String>, _>("namespace").ok().flatten().unwrap_or_default(),
                    "summary": row.try_get::<Option<String>, _>("summary").ok().flatten().unwrap_or_default(),
                    "provider": row.try_get::<Option<String>, _>("provider_type").ok().flatten().unwrap_or_default(),
                    "connection": row.try_get::<Option<String>, _>("connection_name").ok().flatten().unwrap_or_default(),
                    "members": members,
                    "labels": conversation_tags
                }),
            );
            if let Some(person_id) = person_id {
                if let Ok(person) = Person::get(pool, &profile.id, &person_id).await {
                    context.insert(
                        "person".into(),
                        json!({ "id": person.id, "name": person.display_name, "first_name": first_name(&person.display_name), "email": person.primary_email, "is_human": person.is_human, "providers": person.provider_types }),
                    );
                }
            }
            let messages = sqlx::query(
                "SELECT sender_name, sender_email, timestamp, content, content_type FROM message WHERE conversation_id = ?1 AND deleted_datetime IS NULL ORDER BY timestamp DESC LIMIT 40",
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to load messages: {}", e))?;
            let mut rendered: Vec<Value> = messages
                .into_iter()
                .map(|m| {
                    let content: Option<String> = m.get("content");
                    let content_type: Option<String> = m.get("content_type");
                    let text = crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), content_type.as_deref().unwrap_or("text"));
                    let timestamp: String = m.get("timestamp");
                    json!({
                        "sender": m.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
                        "email": m.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default(),
                        "time": timestamp.chars().take(16).collect::<String>().replace('T', " "),
                        "content": text.chars().take(600).collect::<String>()
                    })
                })
                .collect();
            rendered.reverse();
            context.insert("messages".into(), Value::Array(rendered));
        }
    }
    if !context.contains_key("conversation") {
        context.insert("conversation".into(), json!({ "name": "", "type": "", "provider": "", "connection": "", "namespace": "", "summary": "", "members": [], "labels": [] }));
        context.insert("messages".into(), Value::Array(Vec::new()));
    }
    Ok(Value::Object(context))
}

pub fn merge(base: &mut Value, extra: Value) {
    if let (Value::Object(target), Value::Object(source)) = (base, extra) {
        for (k, v) in source {
            match target.get_mut(&k) {
                Some(existing) if existing.is_object() && v.is_object() => merge(existing, v),
                _ => {
                    target.insert(k, v);
                }
            }
        }
    }
}

pub async fn prepare(pool: &Pool<Sqlite>, key: &str, overrides: &Value, conversation_id: Option<&str>, extra: Value) -> Result<(Profile, Prompt, Value, String, String, Option<Value>), String> {
    let profile = Profile::get_active(pool).await?.ok_or_else(|| "No active profile".to_string())?;
    ensure_defaults(pool, &profile.id).await?;
    let spec = prompt(key, overrides)?;
    let mut context = base_context(pool, &profile, conversation_id).await?;
    merge(&mut context, extra);
    let system = render(&spec.system, &context)?;
    let prompt = render(&spec.template, &context)?;
    let schema = match &spec.output_schema {
        Some(raw) => {
            let rendered = render(raw, &context)?;
            Some(serde_json::from_str::<Value>(&rendered).map_err(|e| format!("The output schema of {} is not valid JSON once rendered: {}", key, e))?)
        }
        None => None,
    };
    Ok((profile, spec, context, system, prompt, schema))
}

pub async fn run(pool: &Pool<Sqlite>, key: &str, overrides: &Value, conversation_id: Option<&str>, extra: Value, ai: Option<&AiChoice>) -> Result<PromptRun, String> {
    if !crate::services::assistant_service::enabled(pool).await {
        return Err(action_script::REQUIREMENT_AI_ENABLED.error());
    }
    let (profile, spec, _, system, prompt, schema) = prepare(pool, key, overrides, conversation_id, extra).await?;
    let (connection, model) = get_connection(pool, &profile.id, key, spec.connection_id.as_deref(), spec.model.as_deref(), ai).await?;
    execute(pool, &connection, key, &system, &prompt, model.as_deref(), schema.as_ref()).await
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeatureChoice {
    pub feature: Option<&'static app_setting::ModelFeature>,
    pub connection_id: Option<String>,
    pub model: Option<String>,
}

impl FeatureChoice {
    pub fn get_model(&self, node_connection_id: Option<&str>, connection: &IntelligenceConnection) -> Option<String> {
        let owned = match &self.connection_id {
            Some(own) => *own == connection.id,
            None => node_connection_id.is_none() || connection.is_default,
        };
        self.model.clone().filter(|m| owned && !m.trim().is_empty()).or_else(|| self.feature.and_then(|f| f.get_default_model(&connection.kind)).map(String::from))
    }
}

pub async fn get_feature_choice(pool: &Pool<Sqlite>, key: &str, ai: Option<&AiChoice>) -> Result<FeatureChoice, String> {
    let feature = app_setting::get_model_feature(key);
    if let Some(ai) = ai {
        let filled = |value: &Option<String>| value.clone().filter(|v| !v.trim().is_empty());
        return Ok(FeatureChoice { feature, connection_id: filled(&ai.connection_id), model: filled(&ai.model) });
    }
    let Some(feature) = feature else { return Ok(FeatureChoice::default()) };
    Ok(FeatureChoice {
        feature: Some(feature),
        connection_id: AppSetting::get(pool, &app_setting::feature_key(feature.id, app_setting::FEATURE_CONNECTION)).await?,
        model: AppSetting::get(pool, &app_setting::feature_key(feature.id, app_setting::FEATURE_MODEL)).await?,
    })
}

pub fn get_model(node: Option<&str>, feature: Option<&str>, connection: Option<&str>) -> Option<String> {
    [node, feature, connection].into_iter().flatten().map(str::trim).find(|m| !m.is_empty()).map(String::from)
}

pub async fn get_connection(pool: &Pool<Sqlite>, profile_id: &str, key: &str, connection_id: Option<&str>, model: Option<&str>, ai: Option<&AiChoice>) -> Result<(IntelligenceConnection, Option<String>), String> {
    let choice = get_feature_choice(pool, key, ai).await?;
    let wanted = connection_id.map(String::from).or_else(|| choice.connection_id.clone());
    let chosen = match &wanted {
        Some(id) => IntelligenceConnection::get(pool, id).await?,
        None => None,
    };
    let connection = match chosen {
        Some(connection) => Some(connection),
        None => IntelligenceConnection::default_for(pool, profile_id).await?,
    }
    .ok_or_else(|| action_script::REQUIREMENT_AI_CONNECTION.error())?;
    let connection = describe(pool, connection).await;
    let feature = choice.get_model(connection_id, &connection);
    let model = get_model(model, feature.as_deref(), connection.setting("model").as_deref());
    Ok((connection, model))
}

pub async fn execute(pool: &Pool<Sqlite>, connection: &IntelligenceConnection, key: &str, system: &str, prompt: &str, model: Option<&str>, schema: Option<&Value>) -> Result<PromptRun, String> {
    let origin = origin_for(connection).map_err(|e| classify(&connection.kind, e))?;
    let _permit = crate::services::assistant_service::slot().acquire().await.map_err(|_| "The intelligence queue is closed".to_string())?;
    let started = std::time::Instant::now();
    let answer = origin.run(system, prompt, model, schema, timeout_for(pool).await).await.map_err(|e| classify(&connection.kind, e))?;
    let run = PromptRun {
        key: key.to_string(),
        connection: connection.name.clone(),
        model: model.map(String::from),
        rendered_prompt: prompt.to_string(),
        output: answer.output,
        cost_usd: answer.cost_usd,
        duration_ms: started.elapsed().as_millis() as u64,
    };
    sqlx::query("INSERT INTO assistant_run (id, kind, model, prompt, output, cost_usd, duration_ms, created_at, connection) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")
        .bind(Uuid::new_v4().to_string())
        .bind(key)
        .bind(run.model.clone().unwrap_or_default())
        .bind(prompt)
        .bind(run.output.to_string())
        .bind(run.cost_usd)
        .bind(run.duration_ms as i64)
        .bind(Utc::now().to_rfc3339())
        .bind(&connection.name)
        .execute(pool)
        .await
        .map_err(|e| format!("The answer arrived but the run could not be logged: {}", e))?;
    Ok(run)
}

pub fn classify(kind: &str, error: String) -> String {
    if action_script::unmet_of(&error).is_some() {
        return error;
    }
    let lower = error.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    let access = has(&["not logged in", "/login", "log in", "logged out", "unauthorized", "unauthorised", "unauthenticated", "forbidden", "permission", "api key", "api_key", "invalid token", "invalid x-api-key", "authentication", "oauth"]);
    let found = match kind {
        KIND_CLAUDE_CLI if has(&["could not start", "was not found", "no such file", "is claude code installed"]) => Some(action_script::REQUIREMENT_CLAUDE_INSTALLED),
        KIND_CLAUDE_CLI if access => Some(action_script::REQUIREMENT_CLAUDE_SIGNED_IN),
        KIND_OLLAMA if has(&["model"]) && has(&["not found", "pull"]) => Some(action_script::REQUIREMENT_OLLAMA_MODEL),
        KIND_OLLAMA if has(&["did not answer", "connection refused", "error sending request", "not running", "answered unreadably"]) => Some(action_script::REQUIREMENT_OLLAMA_RUNNING),
        _ if access => Some(action_script::REQUIREMENT_AI_ACCESS),
        _ => None,
    };
    found.map(|r| r.error()).unwrap_or(error)
}


pub fn one_line(text: &str, max: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let line = line.trim_end_matches('.').to_string();
    if line.chars().count() <= max {
        return line;
    }
    let mut cut: String = line.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

pub const SECTION_PROMPT: &str = "summarise_thread";
const SECTION_TEXT_LIMIT: usize = 800;

fn section_kind() -> String {
    SECTION_KIND_SECTION.to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SectionInput {
    #[serde(default = "section_kind")]
    pub kind: String,
    pub start_message_id: String,
    pub end_message_id: String,
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetSectionSummariesInput {
    pub scope: String,
    pub sections: Vec<SectionInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostSectionSummaryInput {
    pub scope: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "section_kind")]
    pub kind: String,
    pub start_message_id: String,
    pub end_message_id: String,
    pub message_ids: Vec<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub options: SummaryOptions,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SummaryOptions {
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub include: Option<String>,
    #[serde(default)]
    pub exclude: Option<String>,
    #[serde(default)]
    pub without_files: bool,
}

impl SummaryOptions {
    fn chosen(&self) -> bool {
        [&self.focus, &self.include, &self.exclude].iter().any(|o| o.as_deref().is_some_and(|s| !s.trim().is_empty())) || self.without_files
    }

    fn pattern(value: &Option<String>) -> Result<Option<Regex>, String> {
        match value.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(p) => regex::RegexBuilder::new(p).case_insensitive(true).build().map(Some).map_err(|e| format!("The filter {} is not a pattern it can read: {}", p, e)),
            None => Ok(None),
        }
    }

    pub fn select(&self, lines: Vec<SectionLine>) -> Result<Vec<SectionLine>, String> {
        let include = Self::pattern(&self.include)?;
        let exclude = Self::pattern(&self.exclude)?;
        let haystack = |line: &SectionLine| format!("{} {} {} {}", line.sender, line.email, line.text, line.files.join(" "));
        Ok(lines
            .into_iter()
            .map(|mut line| {
                if self.without_files {
                    line.files.clear();
                }
                line
            })
            .filter(|line| include.as_ref().is_none_or(|r| r.is_match(&haystack(line))) && exclude.as_ref().is_none_or(|r| !r.is_match(&haystack(line))))
            .collect())
    }
}

pub const SUMMARY_TOO_SMALL: &str = "A summary needs at least two messages, or one message with a file";

pub fn summarisable(lines: &[SectionLine]) -> bool {
    lines.len() >= 2 || lines.iter().any(|line| !line.files.is_empty())
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionSummaries {
    pub summaries: Vec<SectionSummary>,
    pub requirements: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionLine {
    pub id: String,
    pub sender: String,
    pub email: String,
    pub timestamp: String,
    pub modified: String,
    pub text: String,
    pub files: Vec<String>,
}

pub async fn section_lines(pool: &Pool<Sqlite>, ids: &[String]) -> Result<Vec<SectionLine>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let marks = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT id, sender_name, sender_email, timestamp, last_modified_datetime, content, content_type, attachments FROM message WHERE deleted_datetime IS NULL AND id IN ({}) ORDER BY timestamp ASC, id ASC",
        marks
    );
    let mut query = sqlx::query(&sql);
    for id in ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await.map_err(|e| format!("Failed to load the section: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let content: Option<String> = row.try_get("content").ok().flatten();
            let content_type: Option<String> = row.try_get("content_type").ok().flatten();
            SectionLine {
                id: row.get("id"),
                sender: row.try_get::<Option<String>, _>("sender_name").ok().flatten().unwrap_or_default(),
                email: row.try_get::<Option<String>, _>("sender_email").ok().flatten().unwrap_or_default(),
                timestamp: row.get("timestamp"),
                modified: row.try_get::<Option<String>, _>("last_modified_datetime").ok().flatten().unwrap_or_default(),
                text: crate::utils::text_extraction::extract_text_from_content(content.as_deref().unwrap_or(""), content_type.as_deref().unwrap_or("text")),
                files: row.try_get::<Option<String>, _>("attachments").ok().flatten().and_then(|raw| serde_json::from_str::<Vec<Value>>(&raw).ok()).unwrap_or_default().iter().filter(|a| !a.get("is_inline").and_then(Value::as_bool).unwrap_or(false)).filter_map(|a| a.get("name").or_else(|| a.get("filename")).and_then(Value::as_str).map(String::from)).collect(),
            }
        })
        .collect())
}

async fn summary_style(pool: &Pool<Sqlite>, options: &SummaryOptions) -> Value {
    let read = |key: &'static str, fallback: &'static str| async move { AppSetting::get(pool, key).await.ok().flatten().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| fallback.to_string()) };
    let length = match read(app_setting::SUMMARIES_LENGTH, "medium").await.as_str() {
        "short" => "at most two short lines",
        "long" => "a thorough account in up to ten lines",
        _ => "at most five short lines",
    };
    let form = if read(app_setting::SUMMARIES_FORM, "prose").await == "bullets" { "as short bullet points, one fact per line" } else { "as plain prose" };
    let language = if read(app_setting::SUMMARIES_LANGUAGE, "conversation").await == "profile" { "in the language of the app" } else { "in the language the conversation uses" };
    let actions = read(app_setting::SUMMARIES_ACTIONS, "true").await != "false";
    json!({
        "today": Local::now().format("%A %Y-%m-%d").to_string(),
        "style": { "length": length, "form": form, "language": language, "actions": actions, "instructions": read(app_setting::SUMMARIES_INSTRUCTIONS, "").await },
        "focus": options.focus.as_deref().map(str::trim).unwrap_or_default()
    })
}

pub fn section_hash(lines: &[SectionLine]) -> String {
    let mut material = String::new();
    for line in lines {
        material.push_str(&line.id);
        material.push('\u{1f}');
        material.push_str(&line.modified);
        material.push('\u{1f}');
        material.push_str(&line.text);
        material.push('\u{1f}');
        material.push_str(&line.files.join("\u{1f}"));
        material.push('\u{1e}');
    }
    crate::utils::content_hash(material.as_bytes())
}

fn section_time(timestamp: &str) -> String {
    timestamp.chars().take(16).collect::<String>().replace('T', " ")
}

pub fn section_input(lines: &[SectionLine]) -> Value {
    let messages: Vec<Value> = lines
        .iter()
        .map(|line| {
            json!({
                "sender": if line.sender.is_empty() { line.email.clone() } else { line.sender.clone() },
                "email": line.email,
                "time": section_time(&line.timestamp),
                "content": line.text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(SECTION_TEXT_LIMIT).collect::<String>(),
                "files": line.files
            })
        })
        .collect();
    json!({
        "messages": messages,
        "section": {
            "from": lines.first().map(|l| section_time(&l.timestamp)).unwrap_or_default(),
            "to": lines.last().map(|l| section_time(&l.timestamp)).unwrap_or_default(),
            "count": lines.len()
        }
    })
}

pub async fn section_requirements(pool: &Pool<Sqlite>, profile_id: &str) -> Result<Vec<Diagnostic>, String> {
    let state = crate::services::action_service::requirement_state(pool, profile_id).await?;
    Ok(action_script::function_requirements("summarize").iter().filter(|key| !state.met(key)).filter_map(|key| action_script::requirement(key)).map(|r| Diagnostic::requirement(&r)).collect())
}

pub async fn get_section_summaries(pool: &Pool<Sqlite>, profile_id: &str, input: &GetSectionSummariesInput) -> Result<SectionSummaries, String> {
    let mut summaries = Vec::new();
    for section in &input.sections {
        let Some(found) = SectionSummary::get(pool, profile_id, &input.scope, &section.kind, &section.start_message_id).await? else {
            continue;
        };
        let lines = section_lines(pool, &section.message_ids).await?;
        summaries.push(found.checked(&section.end_message_id, &section_hash(&lines)));
    }
    Ok(SectionSummaries { summaries, requirements: section_requirements(pool, profile_id).await? })
}

pub async fn post_section_summary(pool: &Pool<Sqlite>, profile_id: &str, input: &PostSectionSummaryInput) -> Result<SectionSummary, String> {
    if ![SECTION_KIND_SECTION, SECTION_KIND_ALL, SECTION_KIND_SELECTION].contains(&input.kind.as_str()) {
        return Err(format!("There is no section kind called {}", input.kind));
    }
    let lines = input.options.select(section_lines(pool, &input.message_ids).await?)?;
    if lines.is_empty() {
        return Err("Nothing in that part matches the filter".to_string());
    }
    if !summarisable(&lines) {
        return Err(SUMMARY_TOO_SMALL.to_string());
    }
    let hash = section_hash(&lines);
    if let Some(found) = SectionSummary::get(pool, profile_id, &input.scope, &input.kind, &input.start_message_id).await? {
        let found = found.checked(&input.end_message_id, &hash);
        if !found.stale && !input.force && !input.options.chosen() {
            return Ok(found);
        }
    }
    let missing = section_requirements(pool, profile_id).await?;
    if let Some(first) = missing.first().and_then(|d| d.setting.as_deref()).and_then(action_script::requirement) {
        return Err(first.error());
    }
    let mut extra = section_input(&lines);
    merge(&mut extra, summary_style(pool, &input.options).await);
    if let (None, Some(title)) = (&input.conversation_id, input.title.as_deref().filter(|t| !t.trim().is_empty())) {
        merge(&mut extra, json!({ "conversation": { "name": title } }));
    }
    let run = run(pool, SECTION_PROMPT, &Value::Null, input.conversation_id.as_deref(), extra, None).await?;
    let summary = run.output.get("summary").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| "The answer came back without a summary".to_string())?;
    let action_items: Vec<String> = run.output.get("action_items").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let saved = SectionSummary::put(
        pool,
        profile_id,
        PutSectionSummary {
            scope: &input.scope,
            kind: &input.kind,
            start_message_id: &input.start_message_id,
            end_message_id: &input.end_message_id,
            content_hash: &hash,
            message_count: lines.len() as i64,
            summary,
            action_items: &action_items,
            connection: &run.connection,
            model: run.model.as_deref(),
        },
    )
    .await?;
    Ok(saved.checked(&input.end_message_id, &hash))
}


pub async fn test_connection(pool: &Pool<Sqlite>, connection_id: &str) -> Result<String, String> {
    let connection = IntelligenceConnection::get(pool, connection_id).await?.ok_or_else(|| "Connection not found".to_string())?;
    if connection.kind == KIND_CLAUDE_CLI {
        claude_status(true).await;
    }
    let mut connection = describe(pool, connection).await;
    if connection.kind == KIND_OLLAMA {
        ollama_status(&connection.setting("host").unwrap_or_default(), true).await;
        connection = describe(pool, connection).await;
    }
    if connection.kind == KIND_REPLICATE {
        let token = connection.setting("api_token").unwrap_or_default();
        let body: Value = reqwest::Client::new()
            .get("https://api.replicate.com/v1/account")
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("Replicate did not answer: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Replicate answered unreadably: {}", e))?;
        if let Some(detail) = body["detail"].as_str() {
            return Err(format!("Replicate refused: {}", detail));
        }
        return Ok(format!("Signed in to Replicate as {}", body["username"].as_str().unwrap_or("an account")));
    }
    if !connection.available {
        return Err(connection.detail.unwrap_or_else(|| "The connection is not available".to_string()));
    }
    let origin = origin_for(&connection)?;
    let schema = json!({ "type": "object", "properties": { "ok": { "type": "boolean" } }, "required": ["ok"] });
    let model = connection.setting("model");
    let answer = origin
        .run("Answer with JSON {\"ok\": true} and nothing else.", "Say ok.", model.as_deref(), Some(&schema), timeout_for(pool).await)
        .await?;
    if answer.output["ok"].as_bool() == Some(true) || answer.output.is_string() {
        Ok(format!("{} answered{}", connection.name, model.map(|m| format!(" with {}", m)).unwrap_or_default()))
    } else {
        Ok(format!("{} answered unexpectedly: {}", connection.name, answer.output))
    }
}

pub async fn models(pool: &Pool<Sqlite>, connection_id: &str) -> Result<Vec<String>, String> {
    let connection = IntelligenceConnection::get(pool, connection_id).await?.ok_or_else(|| "Connection not found".to_string())?;
    let connection = describe(pool, connection).await;
    if connection.kind == KIND_CLAUDE_CLI {
        return Ok(connection.models);
    }
    origin_for(&connection)?.models().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_pool, test_profile};

    #[test]
    fn openai_urls_add_v1_only_when_the_base_has_no_version() {
        let at = |base: &str| OpenAiCompatible { base_url: base.into(), api_key: String::new(), model: String::new() }.url("chat/completions");
        assert_eq!(at("http://localhost:11434"), "http://localhost:11434/v1/chat/completions");
        assert_eq!(at("http://localhost:1234/"), "http://localhost:1234/v1/chat/completions");
        assert_eq!(at("https://api.openai.com/v1"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(at("https://example.org/openai/v2/"), "https://example.org/openai/v2/chat/completions");
        assert_eq!(at("https://example.org/vision"), "https://example.org/vision/v1/chat/completions");
    }

    #[test]
    fn ollama_tags_parse_into_models() {
        let body = json!({ "models": [
            { "name": "qwen3:8b", "model": "qwen3:8b", "size": 5225388164u64, "details": { "family": "qwen3", "parameter_size": "8.2B" } },
            { "model": "llama3.2:3b", "size": 2019393189u64, "details": {} },
            { "size": 1 }
        ] });
        let models = parse_tags(&body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], OllamaModel { name: "qwen3:8b".into(), size_bytes: 5225388164, parameter_size: Some("8.2B".into()), family: Some("qwen3".into()) });
        assert_eq!(models[1].name, "llama3.2:3b");
        assert!(parse_tags(&json!({})).is_empty());
    }

    #[test]
    fn suggestions_offer_only_what_a_running_ollama_lacks() {
        let installed = vec![OllamaModel { name: "qwen3:8b".into(), size_bytes: 0, parameter_size: None, family: None }];
        let status = |running: bool| OllamaStatus { installed: true, binary: None, running, version: running.then(|| "0.12.3".to_string()), host: "http://localhost:11434".into(), models: if running { installed.clone() } else { Vec::new() }, download_url: OLLAMA_DOWNLOAD.into() };
        let list = suggestions(&status(true));
        assert!(list.len() >= 4);
        assert!(list.iter().all(|s| s.command == format!("ollama pull {}", s.name) && !s.size.is_empty() && !s.good_for.is_empty()));
        assert!(list.iter().all(|s| s.name != "qwen3:8b"));
        assert!(list.iter().any(|s| s.name == "llama3.2:3b"));
        assert!(suggestions(&status(false)).is_empty());
    }

    #[test]
    fn pull_lines_become_progress() {
        let p = parse_pull_line("qwen3:8b", r#"{"status":"pulling 1a2b","digest":"sha256:1a2b","total":100,"completed":40}"#).unwrap();
        assert_eq!((p.status.as_str(), p.completed, p.total, p.done), ("pulling 1a2b", Some(40), Some(100), false));
        assert!(parse_pull_line("qwen3:8b", r#"{"status":"success"}"#).unwrap().done);
        let failed = parse_pull_line("nope", r#"{"error":"pull model manifest: file does not exist"}"#).unwrap();
        assert!(failed.done && failed.error.is_some());
        assert!(parse_pull_line("x", "not json").is_none());
    }

    #[tokio::test]
    async fn a_pull_refuses_names_that_are_not_models() {
        assert!(pull_ollama_model(app_setting::get_default(app_setting::OLLAMA_HOST).unwrap(), "qwen3; echo hi", |_| {}).await.unwrap_err().contains("not a model name"));
        assert!(pull_ollama_model(app_setting::get_default(app_setting::OLLAMA_HOST).unwrap(), "", |_| {}).await.is_err());
    }

    #[test]
    fn the_claude_account_comes_from_its_auth_status() {
        assert_eq!(claude_account(&json!({ "loggedIn": true, "authMethod": "claude.ai", "email": "me@example.org", "orgName": "Mine" })).as_deref(), Some("me@example.org"));
        assert_eq!(claude_account(&json!({ "loggedIn": true, "email": "", "orgName": "Mine" })).as_deref(), Some("Mine"));
        assert_eq!(claude_account(&json!({ "loggedIn": false, "authMethod": "none" })), None);
        assert_eq!(claude_account(&Value::Null), None);
    }

    #[test]
    fn claude_models_come_from_its_help_and_settings() {
        let help = "  --mcp-config <configs...>             Load MCP servers\n  --model <model>                       Model for the current session. Provide\n                                        an alias for the latest model (e.g.\n                                        'fable', 'opus', or 'sonnet') or a\n                                        model's full name.\n  -n, --name <name>                     Set a display name ('ignored')\n";
        assert_eq!(claude_models(help, &Value::Null), (None, vec!["fable".to_string(), "opus".into(), "sonnet".into()]));
        assert_eq!(claude_models(help, &json!({ "model": "opus" })), (Some("opus".to_string()), vec!["opus".to_string(), "fable".into(), "sonnet".into()]));
        assert_eq!(claude_models(help, &json!({ "env": { "ANTHROPIC_MODEL": "custom-1" } })).1.first().map(String::as_str), Some("custom-1"));
        assert_eq!(claude_models(help, &json!({ "availableModels": ["sonnet", " ", "haiku"] })).1, vec!["sonnet".to_string(), "haiku".into()]);
        assert_eq!(claude_models("Usage: claude", &Value::Null), (None, Vec::new()));
    }

    #[tokio::test]
    async fn an_ollama_connection_defaults_to_the_local_host() {
        let connection = IntelligenceConnection { id: "c".into(), profile_id: "p".into(), kind: KIND_OLLAMA.into(), name: "Ollama".into(), config: std::collections::BTreeMap::new(), is_default: false, available: false, detail: None, models: vec![], created_at: String::new() };
        assert!(origin_for(&connection).is_ok());
        let ollama = Ollama { host: app_setting::get_default(app_setting::OLLAMA_HOST).unwrap().into(), model: "qwen3:8b".into() };
        assert_eq!(ollama.chat().url("chat/completions"), "http://localhost:11434/v1/chat/completions");
    }

    #[test]
    fn one_line_collapses_whitespace_and_caps_length() {
        assert_eq!(one_line("  Vil ha\n fast pris   for drift.  ", 120), "Vil ha fast pris for drift");
        let long = one_line(&"ord ".repeat(60), 20);
        assert_eq!(long.chars().count(), 20);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn failures_that_a_setting_fixes_become_requirements() {
        let setting = |kind: &str, error: &str| action_script::unmet_of(&classify(kind, error.to_string())).map(|d| (d.setting.unwrap_or_default(), d.message));
        assert_eq!(setting(KIND_CLAUDE_CLI, "Claude refused: Not logged in · Please run /login"), Some(("ai.claude".to_string(), "Claude Code signed in".to_string())));
        assert_eq!(setting(KIND_CLAUDE_CLI, "Could not start claude: No such file or directory (os error 2). Is Claude Code installed and on the path?").map(|s| s.1), Some("Claude Code installed".to_string()));
        assert_eq!(setting(KIND_OLLAMA, "http://localhost:11434 did not answer: error sending request").map(|s| s.0), Some("ai.ollama".to_string()));
        assert_eq!(setting(KIND_OLLAMA, "http://localhost:11434 refused: model \"qwen3:8b\" not found, try pulling it first").map(|s| s.1), Some("The model pulled in Ollama".to_string()));
        assert_eq!(setting(KIND_REPLICATE, "Replicate refused: Unauthenticated").map(|s| s.0), Some("ai.connection".to_string()));
        assert_eq!(setting(KIND_OPENAI_COMPATIBLE, &format!("Draft: {}", action_script::REQUIREMENT_AI_MODEL.error())).map(|s| s.0), Some("ai.default-model".to_string()));
        assert_eq!(setting(KIND_CLAUDE_CLI, "Claude took too long and was stopped"), None);
    }

    fn line(id: &str, sender: &str, text: &str) -> SectionLine {
        SectionLine { id: id.into(), sender: sender.into(), email: format!("{}@client.example", sender.to_lowercase()), timestamp: "2026-10-14T09:30:00+02:00".into(), modified: "2026-10-14T09:30:00+02:00".into(), text: text.into(), files: Vec::new() }
    }

    #[test]
    fn the_section_hash_follows_ids_edits_and_order() {
        let a = vec![line("m1", "Kari", "Hei"), line("m2", "Ola", "Kan vi flytte møtet?")];
        assert_eq!(section_hash(&a), section_hash(&a.clone()));
        let mut edited = a.clone();
        edited[1].text = "Kan vi flytte møtet til fredag?".into();
        assert_ne!(section_hash(&a), section_hash(&edited));
        let mut touched = a.clone();
        touched[0].modified = "2026-10-14T10:00:00+02:00".into();
        assert_ne!(section_hash(&a), section_hash(&touched));
        assert_ne!(section_hash(&a), section_hash(&[a[1].clone(), a[0].clone()]));
        assert_ne!(section_hash(&a), section_hash(&a[..1]));
    }

    #[test]
    fn the_filter_picks_messages_and_one_message_needs_a_file() {
        let lines = vec![line("m1", "Kari", "Faktura for oktober"), line("m2", "Ola", "Lunsj?"), SectionLine { files: vec!["kubeconfig.yaml".into()], ..line("m3", "Kari", "Her er tilgangen") }];
        let only = |include: &str, exclude: &str| SummaryOptions { include: Some(include.into()), exclude: Some(exclude.into()), ..Default::default() }.select(lines.clone()).unwrap().iter().map(|l| l.id.clone()).collect::<Vec<_>>();
        assert_eq!(only("kari", ""), vec!["m1", "m3"]);
        assert_eq!(only("", "lunsj"), vec!["m1", "m3"]);
        assert_eq!(only("kubeconfig", ""), vec!["m3"]);
        assert!(SummaryOptions { include: Some("(".into()), ..Default::default() }.select(lines.clone()).is_err());
        assert!(!summarisable(&lines[..1]));
        assert!(summarisable(&lines[2..]));
        assert!(summarisable(&lines[..2]));
        let stripped = SummaryOptions { without_files: true, ..Default::default() }.select(lines[2..].to_vec()).unwrap();
        assert!(!summarisable(&stripped));
        assert!(SummaryOptions { focus: Some("budget".into()), ..Default::default() }.chosen());
        assert!(!SummaryOptions::default().chosen());
    }

    #[test]
    fn the_section_prompt_is_built_from_the_section_messages() {
        let lines = vec![line("m1", "Kari", "Hei,\n\n  kan du se   på avtalen?"), SectionLine { sender: String::new(), ..line("m2", "Ola", &"ord ".repeat(400)) }];
        let input = section_input(&lines);
        assert_eq!(input["section"], json!({ "from": "2026-10-14 09:30", "to": "2026-10-14 09:30", "count": 2 }));
        assert_eq!(input["messages"][0], json!({ "sender": "Kari", "email": "kari@client.example", "time": "2026-10-14 09:30", "content": "Hei, kan du se på avtalen?", "files": [] }));
        assert_eq!(input["messages"][1]["sender"], json!("ola@client.example"));
        assert_eq!(input["messages"][1]["content"].as_str().unwrap().chars().count(), SECTION_TEXT_LIMIT);
        let seed = seeds().into_iter().find(|s| s.key == SECTION_PROMPT).unwrap();
        let mut context = json!({ "profile": { "name": "Milo" }, "conversation": { "name": "Avtale", "type": "email", "provider": "google", "members": [] }, "messages": [ { "time": "old", "sender": "x", "content": "outside the section" } ] });
        merge(&mut context, input);
        let prompt = render(seed.template, &context).unwrap();
        assert!(prompt.contains("[2026-10-14 09:30] Kari: Hei, kan du se på avtalen?\n"));
        assert!(!prompt.contains("outside the section"));
    }

    async fn section_fixture() -> (Pool<Sqlite>, String, String, String, Vec<String>) {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = crate::test_helpers::test_provider(&pool, &profile, "mock").await;
        let conversation = crate::test_helpers::test_conversation(&pool, "Avtale", "group_message").await;
        let mut ids = Vec::new();
        for (i, text) in ["Hei", "Kan vi flytte møtet?"].iter().enumerate() {
            let id = crate::test_helpers::test_message(&pool, &conversation, &provider, text).await;
            sqlx::query("UPDATE message SET timestamp = ?1 WHERE id = ?2").bind(format!("2026-10-14T09:3{}:00+02:00", i)).bind(&id).execute(&pool).await.unwrap();
            ids.push(id);
        }
        (pool, profile, provider, conversation, ids)
    }

    #[tokio::test]
    async fn an_unchanged_section_answers_from_the_cache() {
        let (pool, profile, _, conversation, ids) = section_fixture().await;
        let lines = section_lines(&pool, &ids).await.unwrap();
        assert_eq!(lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), vec!["Hei", "Kan vi flytte møtet?"]);
        let hash = section_hash(&lines);
        let items = vec!["Svar Kari".to_string()];
        SectionSummary::put(&pool, &profile, PutSectionSummary { scope: &conversation, kind: SECTION_KIND_SECTION, start_message_id: &ids[0], end_message_id: &ids[1], content_hash: &hash, message_count: 2, summary: "Kari vil flytte møtet", action_items: &items, connection: "Claude", model: None }).await.unwrap();
        let input = PostSectionSummaryInput { scope: conversation.clone(), conversation_id: Some(conversation.clone()), title: None, kind: SECTION_KIND_SECTION.into(), start_message_id: ids[0].clone(), end_message_id: ids[1].clone(), message_ids: ids.clone(), force: false, options: SummaryOptions::default() };
        let hit = post_section_summary(&pool, &profile, &input).await.unwrap();
        assert_eq!((hit.summary.as_str(), hit.action_items.clone(), hit.stale), ("Kari vil flytte møtet", items, false));
        let forced = post_section_summary(&pool, &profile, &PostSectionSummaryInput { force: true, ..input.clone() }).await;
        assert!(forced.is_err());
        let other = SectionSummary::get(&pool, &profile, &conversation, SECTION_KIND_ALL, &ids[0]).await.unwrap();
        assert!(other.is_none());
    }

    #[tokio::test]
    async fn a_new_message_in_the_section_marks_its_summary_stale() {
        let (pool, profile, provider, conversation, mut ids) = section_fixture().await;
        let hash = section_hash(&section_lines(&pool, &ids).await.unwrap());
        SectionSummary::put(&pool, &profile, PutSectionSummary { scope: &conversation, kind: SECTION_KIND_SECTION, start_message_id: &ids[0], end_message_id: &ids[1], content_hash: &hash, message_count: 2, summary: "Kari vil flytte møtet", action_items: &[], connection: "Claude", model: None }).await.unwrap();
        let section = |ids: &[String]| SectionInput { kind: SECTION_KIND_SECTION.into(), start_message_id: ids[0].clone(), end_message_id: ids[ids.len() - 1].clone(), message_ids: ids.to_vec() };
        let fresh = get_section_summaries(&pool, &profile, &GetSectionSummariesInput { scope: conversation.clone(), sections: vec![section(&ids)] }).await.unwrap();
        assert_eq!(fresh.summaries.iter().map(|s| s.stale).collect::<Vec<_>>(), vec![false]);
        let arrived = crate::test_helpers::test_message(&pool, &conversation, &provider, "Fredag passer").await;
        sqlx::query("UPDATE message SET timestamp = '2026-10-14T09:40:00+02:00' WHERE id = ?1").bind(&arrived).execute(&pool).await.unwrap();
        ids.push(arrived);
        let after = get_section_summaries(&pool, &profile, &GetSectionSummariesInput { scope: conversation.clone(), sections: vec![section(&ids)] }).await.unwrap();
        assert_eq!(after.summaries.iter().map(|s| (s.stale, s.summary.as_str())).collect::<Vec<_>>(), vec![(true, "Kari vil flytte møtet")]);
        sqlx::query("UPDATE message SET content = 'Hei igjen' WHERE id = ?1").bind(&ids[0]).execute(&pool).await.unwrap();
        let edited = get_section_summaries(&pool, &profile, &GetSectionSummariesInput { scope: conversation.clone(), sections: vec![section(&ids[..2])] }).await.unwrap();
        assert!(edited.summaries[0].stale);
        assert!(edited.requirements.iter().all(|d| d.severity == "requirement" && d.setting.is_some()));
    }

    #[test]
    fn the_mail_summary_seed_renders_the_message() {
        let seed = seeds().into_iter().find(|s| s.key == "summarise_mail").unwrap();
        let context = json!({ "profile": { "name": "Milo" }, "message": { "from_name": "Anna", "from_email": "anna@client.example", "subject": "Avtale", "text": "Kan vi gå for fast pris?" } });
        assert!(render(seed.system, &context).unwrap().contains("80 characters"));
        assert!(render(seed.template, &context).unwrap().contains("Kan vi gå for fast pris?"));
        assert!(serde_json::from_str::<Value>(seed.schema.unwrap()).is_ok());
    }


    fn context() -> Value {
        json!({
            "profile": { "name": "Milo" },
            "splits": [ { "name": "Important", "description": "People writing to me" }, { "name": "Other", "description": "The rest" } ],
            "split_names_json": "[\"Important\",\"Other\"]",
            "subject": { "provider_type": "microsoft", "connection_name": "Northwind", "conversation_type": "direct_message", "conversation_name": "Kari", "sender_name": "Kari Nordmann", "sender_email": "kari@acme.no", "subject": "", "member_count": 2, "body": "Hei, kan du se på dette?" },
            "messages": [ { "time": "2026-10-09 10:00", "sender": "Kari", "content": "Hei" } ],
            "instruction": ""
        })
    }

    #[test]
    fn the_sort_prompt_renders_as_before() {
        let seed = seeds().into_iter().find(|s| s.key == "sort").unwrap();
        let system = render(seed.system, &context()).unwrap();
        assert!(system.starts_with("You sort one conversation from Milo's unified inbox into exactly one split. The splits and what belongs in them:\n- Important: People writing to me\n- Other: The rest\nAnswer with the split name"));
        let prompt = render(seed.template, &context()).unwrap();
        assert_eq!(prompt, "Source: microsoft\nConnection: Northwind\nKind: direct_message\nConversation: Kari\nLatest sender: Kari Nordmann <kari@acme.no>\nSubject: \nMembers: 2\n\nLatest message:\nHei, kan du se på dette?");
        let schema: Value = serde_json::from_str(&render(seed.schema.unwrap(), &context()).unwrap()).unwrap();
        assert_eq!(schema["properties"]["label"]["enum"], json!(["Important", "Other"]));
    }

    #[test]
    fn ranges_and_conditions_render() {
        let out = render("{{ range .messages }}[{{ .time }}] {{ .sender }}: {{ .content }}\n{{ end }}{{ if .instruction }}X{{ end }}", &context()).unwrap();
        assert_eq!(out, "[2026-10-09 10:00] Kari: Hei\n");
        assert!(render("{{ .nope", &context()).is_err());
    }

    #[test]
    fn variables_form_a_schema_from_the_template() {
        let vars = variables(r#"Hi {{ .profile.name }} {{ range .messages }}{{ .sender }}: {{ .content | lower }} {{ $.me.name }}{{ end }} {{ if .instruction }}x{{ end }}{{ with .person }}{{ .first_name | default "there" }}{{ end }} {{ " .fake" | lower }}"#);
        let names: Vec<(&str, &str)> = vars.iter().map(|v| (v.name.as_str(), v.kind.as_str())).collect();
        assert_eq!(names, vec![("instruction", VARIABLE_FLAG), ("me.name", VARIABLE_TEXT), ("messages", VARIABLE_LIST), ("person", VARIABLE_FLAG), ("person.first_name", VARIABLE_TEXT), ("profile.name", VARIABLE_TEXT)]);
        let messages = vars.iter().find(|v| v.name == "messages").unwrap();
        assert_eq!(messages.fields, vec!["content", "sender"]);
        assert_eq!(vars.iter().find(|v| v.name == "person.first_name").unwrap().sample, "Kari");
        assert!(variables("plain text, no fields").is_empty());
        let both = variables("{{ if .note }}Note: {{ .note }}{{ else if .other }}-{{ end }}");
        assert_eq!(both.iter().map(|v| (v.name.as_str(), v.kind.as_str())).collect::<Vec<_>>(), vec![("note", VARIABLE_TEXT), ("other", VARIABLE_FLAG)]);
    }

    #[test]
    fn helpers_pipe_lower_upper_default_and_date() {
        let context = json!({ "person": { "name": "Kari Nordmann", "nick": "" }, "at": "2026-10-14T09:30:00+02:00", "day": "2026-03-05" });
        assert_eq!(render("{{ .person.name | lower }} {{ .person.name | upper }}", &context).unwrap(), "kari nordmann KARI NORDMANN");
        assert_eq!(render(r#"{{ .person.nick | default "du" }} {{ default "x" .person.name }} {{ .person.missing | default "ukjent" }}"#, &context).unwrap(), "du Kari Nordmann ukjent");
        assert_eq!(render(r#"{{ .at | date "2 Jan 2006 15:04" }}"#, &context).unwrap(), "14 Oct 2026 09:30");
        assert_eq!(render(r#"{{ .day | date "Monday 02.01.06" }}"#, &context).unwrap(), "Thursday 05.03.26");
        assert_eq!(render(r#"{{ .day | date "%d/%m" }}"#, &context).unwrap(), "05/03");
        assert_eq!(render(r#"{{ "not a date" | date "2006" }}"#, &context).unwrap(), "not a date");
        assert_eq!(go_layout("2006-01-02T15:04:05-07:00"), "%Y-%m-%dT%H:%M:%S%:z");
    }

    #[test]
    fn ranges_conditions_and_missing_values_render_cleanly() {
        let context = json!({ "items": [ { "name": "A" }, { "name": "B" } ], "flag": true });
        assert_eq!(render("{{ range .items }}- {{ .name }}\n{{ end }}{{ if .flag }}yes{{ else }}no{{ end }}", &context).unwrap(), "- A\n- B\nyes");
        assert_eq!(render("[{{ .person.name }}]{{ if .absent }}x{{ else }}y{{ end }}{{ range .none }}z{{ end }}", &json!({ "person": null })).unwrap(), "[]y");
        assert_eq!(missing_variables("{{ .person.name }} {{ .items }}", &context), vec!["person.name"]);
        assert!(check_template("{{ if .x }}").is_err());
        assert!(check_template("{{ .x | nosuchhelper }}").is_err());
        let mut base = json!({ "me": { "name": "Milo", "email": "m@x" }, "keep": 1 });
        merge(&mut base, json!({ "me": { "name": "Ola" }, "extra": true }));
        assert_eq!(base, json!({ "me": { "name": "Ola", "email": "m@x" }, "keep": 1, "extra": true }));
    }

    #[test]
    fn command_args_render_and_split() {
        let origin = CommandOrigin {
            command: "cat".into(),
            args: "--model {{ .model }} --say \"hello world\"".into(),
            stdin: "prompt".into(),
            output: "json".into(),
            model_arg: "-m".into(),
            models_command: String::new(),
            model: "small".into(),
        };
        assert_eq!(origin.argv("s", "p", None, None).unwrap(), vec!["--model", "small", "--say", "hello world", "-m", "small"]);
    }

    #[tokio::test]
    async fn the_command_origin_reads_json_from_stdout() {
        let origin = CommandOrigin {
            command: "cat".into(),
            args: String::new(),
            stdin: "prompt".into(),
            output: "json".into(),
            model_arg: String::new(),
            models_command: "printf 'a\\nb\\n'".into(),
            model: String::new(),
        };
        let answer = origin.run("system", "{\"label\":\"Other\"}", None, None, Duration::from_secs(5)).await.unwrap();
        assert_eq!(answer.output["label"], "Other");
        assert_eq!(origin.models().await.unwrap(), vec!["a", "b"]);
        let echo = CommandOrigin { command: "echo".into(), args: "plain text".into(), stdin: "none".into(), output: "text".into(), model_arg: String::new(), models_command: String::new(), model: String::new() };
        assert_eq!(echo.run("s", "p", None, None, Duration::from_secs(5)).await.unwrap().output, json!("plain text"));
    }

    #[test]
    fn the_openai_body_carries_the_schema() {
        let origin = OpenAiCompatible { base_url: "http://localhost:11434/v1".into(), api_key: String::new(), model: "llama3".into() };
        let schema = json!({ "type": "object" });
        let body = origin.request_body("sys", "hello", None, Some(&schema));
        assert_eq!(body["model"], "llama3");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["content"], "hello");
        assert_eq!(body["response_format"]["json_schema"]["schema"], schema);
        assert!(origin.request_body("sys", "hello", Some("mistral"), None)["response_format"].is_null());
    }

    #[test]
    fn claude_is_not_found_without_a_path() {
        let saved = std::env::var_os("PATH");
        std::env::set_var("PATH", "");
        let found = resolve_binary("claude-that-does-not-exist");
        match saved {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        assert!(found.is_none());
    }

    #[test]
    fn node_overrides_layer_over_the_code_default() {
        let plain = prompt("sort", &Value::Null).unwrap();
        assert!(plain.model.is_none());
        assert!(plain.variables.iter().any(|v| v.name == "subject.body"));
        let tuned = prompt("sort", &json!({ "model": "x", "system": "Be brief", "output_schema": "" })).unwrap();
        assert_eq!(tuned.model.as_deref(), Some("x"));
        assert_eq!(tuned.system, "Be brief");
        assert_eq!(tuned.template, plain.template);
        assert!(tuned.output_schema.is_none());
        assert!(prompt("nothing", &Value::Null).is_err());
        assert_eq!(prompts().len(), seeds().len());
    }

    #[test]
    fn only_changed_fields_become_overrides() {
        let seed = seeds().into_iter().find(|s| s.key == "sort").unwrap();
        assert!(overrides("sort", seed.system, seed.template, seed.schema, None, Some("")).is_none());
        let changed = overrides("sort", "Mine", seed.template, seed.schema, Some("conn"), None).unwrap();
        assert_eq!(changed, json!({ "system": "Mine", "connection_id": "conn" }));
    }

    #[tokio::test]
    async fn the_base_context_carries_the_conversation() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let profile = Profile::get_active(&pool).await.unwrap().unwrap();
        Tag::ensure_defaults(&pool, &profile_id).await.unwrap();
        let conversation = crate::test_helpers::test_conversation(&pool, "Kari", "direct_message").await;
        let context = base_context(&pool, &profile, Some(&conversation)).await.unwrap();
        assert_eq!(context["conversation"]["name"], "Kari");
        assert_eq!(context["profile"]["name"], "Test Profile");
        assert!(context["split_names_json"].as_str().unwrap().contains("Important"));
        assert_eq!(context["messages"].as_array().unwrap().len(), 0);
    }

    fn connection(id: &str, kind: &str, model: Option<&str>, is_default: bool) -> IntelligenceConnection {
        let mut config = std::collections::BTreeMap::new();
        if let Some(model) = model {
            config.insert("model".to_string(), model.to_string());
        }
        IntelligenceConnection { id: id.into(), profile_id: "p".into(), kind: kind.into(), name: id.into(), config, is_default, available: false, detail: None, models: vec![], created_at: String::new() }
    }

    #[test]
    fn a_node_beats_the_feature_and_the_feature_beats_the_connection() {
        assert_eq!(get_model(Some("node"), Some("feature"), Some("connection")).as_deref(), Some("node"));
        assert_eq!(get_model(None, Some("feature"), Some("connection")).as_deref(), Some("feature"));
        assert_eq!(get_model(Some(" "), None, Some("connection")).as_deref(), Some("connection"));
        assert_eq!(get_model(None, Some(""), None), None);
        let summaries = app_setting::get_model_feature("summarise_thread");
        let default = connection("a", KIND_COMMAND, Some("big"), true);
        let other = connection("b", KIND_COMMAND, Some("other"), false);
        let open = FeatureChoice { feature: summaries, connection_id: None, model: Some("small".into()) };
        assert_eq!(open.get_model(None, &default).as_deref(), Some("small"));
        assert_eq!(open.get_model(Some("b"), &other), None);
        let pinned = FeatureChoice { feature: summaries, connection_id: Some("b".into()), model: Some("small".into()) };
        assert_eq!(pinned.get_model(None, &other).as_deref(), Some("small"));
        assert_eq!(pinned.get_model(None, &default), None);
        assert_eq!(FeatureChoice::default().get_model(None, &default), None);
        let voice = FeatureChoice { feature: app_setting::get_model_feature(crate::services::voice_service::PROMPT_KEY), connection_id: None, model: None };
        let claude = connection("c", KIND_CLAUDE_CLI, Some("big"), true);
        assert_eq!(voice.get_model(None, &claude).as_deref(), voice.feature.unwrap().get_default_model(KIND_CLAUDE_CLI));
        assert_eq!(voice.get_model(None, &default), None);
    }

    #[test]
    fn every_prompt_has_at_most_one_feature_and_every_feature_has_prompts() {
        let keys: Vec<&str> = seeds().iter().map(|s| s.key).collect();
        for key in &keys {
            let owners = app_setting::MODEL_FEATURES.iter().filter(|f| f.prompts.contains(key)).count();
            assert!(owners <= 1, "{} belongs to more than one model feature", key);
        }
        for feature in app_setting::MODEL_FEATURES {
            assert!(!feature.prompts.is_empty(), "{} has no prompts", feature.id);
            for prompt in feature.prompts {
                assert!(keys.contains(prompt), "{} lists {} which is no prompt", feature.id, prompt);
            }
            for (kind, model) in feature.models {
                assert!(crate::models::intelligence::KINDS.contains(kind) && !model.is_empty(), "{} has a bad default {} for {}", feature.id, model, kind);
            }
        }
        for key in [crate::services::voice_service::PROMPT_KEY, SECTION_PROMPT, "author_action", "author_template"] {
            assert!(app_setting::get_model_feature(key).is_some(), "{}", key);
        }
        for key in [crate::services::receipt_service::PROMPT_KEY, "sort", "classify_sender", "summarise_mail", "parse_events", "reply_draft", "script_ai", "script_ai_json"] {
            assert!(app_setting::get_model_feature(key).is_none(), "{} is chosen on its action", key);
        }
    }

    #[tokio::test]
    async fn the_feature_setting_and_the_action_choose_connection_and_model_and_a_node_overrides_both() {
        let pool = test_pool().await;
        let profile_id = test_profile(&pool).await;
        let post = |name: &str, model: &str, is_default: bool| {
            let mut config = std::collections::BTreeMap::new();
            config.insert("command".to_string(), "true".to_string());
            config.insert("model".to_string(), model.to_string());
            crate::models::intelligence::PostIntelligenceConnectionInput { kind: KIND_COMMAND.into(), name: name.into(), config, is_default: Some(is_default) }
        };
        let first = IntelligenceConnection::post(&pool, &profile_id, post("First", "first-default", true)).await.unwrap();
        let second = IntelligenceConnection::post(&pool, &profile_id, post("Second", "second-default", false)).await.unwrap();
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", None, None, None).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (first.id.as_str(), Some("first-default")));
        AppSetting::put(&pool, &app_setting::feature_key("threads", app_setting::FEATURE_MODEL), "summary-model").await.unwrap();
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", None, None, None).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (first.id.as_str(), Some("summary-model")));
        let (_, model) = get_connection(&pool, &profile_id, "summarise_mail", None, None, None).await.unwrap();
        assert_eq!(model.as_deref(), Some("first-default"));
        AppSetting::put(&pool, &app_setting::feature_key("threads", app_setting::FEATURE_CONNECTION), &second.id).await.unwrap();
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", None, None, None).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (second.id.as_str(), Some("summary-model")));
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", Some(&first.id), None, None).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (first.id.as_str(), Some("first-default")));
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", None, Some("node-model"), None).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (second.id.as_str(), Some("node-model")));
        AppSetting::put(&pool, &app_setting::feature_key("threads", app_setting::FEATURE_MODEL), "").await.unwrap();
        let (_, model) = get_connection(&pool, &profile_id, "summarise_thread", None, None, None).await.unwrap();
        assert_eq!(model.as_deref(), Some("second-default"));
        let action = AiChoice { connection_id: Some(second.id.clone()), model: Some("action-model".into()) };
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_mail", None, None, Some(&action)).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (second.id.as_str(), Some("action-model")));
        let (chosen, model) = get_connection(&pool, &profile_id, "sort", Some(&first.id), None, Some(&action)).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (first.id.as_str(), Some("first-default")));
        let (chosen, model) = get_connection(&pool, &profile_id, "sort", None, Some("node-model"), Some(&action)).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (second.id.as_str(), Some("node-model")));
        let (chosen, model) = get_connection(&pool, &profile_id, "summarise_thread", None, None, Some(&AiChoice::default())).await.unwrap();
        assert_eq!((chosen.id.as_str(), model.as_deref()), (first.id.as_str(), Some("first-default")));
        IntelligenceConnection::delete(&pool, &second.id).await.unwrap();
        let (chosen, _) = get_connection(&pool, &profile_id, "summarise_thread", None, None, None).await.unwrap();
        assert_eq!(chosen.id, first.id);
    }

    #[tokio::test]
    async fn claude_connections_offer_the_models_the_cli_reports() {
        let pool = test_pool().await;
        let discovered = claude_status(false).await.models;
        let described = describe(&pool, connection("c", KIND_CLAUDE_CLI, Some("picked"), true)).await;
        assert_eq!(described.models.first().map(String::as_str), Some("picked"));
        assert!(discovered.iter().all(|m| described.models.contains(m)));
        let plain = describe(&pool, connection("c", KIND_CLAUDE_CLI, None, true)).await;
        assert_eq!(plain.setting("model"), None);
        assert_eq!(plain.models, discovered);
        AppSetting::put(&pool, app_setting::ASSISTANT_MODEL, "two").await.unwrap();
        assert_eq!(describe(&pool, connection("c", KIND_CLAUDE_CLI, None, true)).await.models.first().map(String::as_str), Some("two"));
    }
}
