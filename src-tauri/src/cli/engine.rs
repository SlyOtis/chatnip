use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Manager, Webview, Wry};

pub const IDENTIFIER: &str = "com.slyotis.chatnip";
pub const SOCKET_NAME: &str = "chatnip.sock";
pub const LOCKED: &str = "Chatnip is locked";
const MAIN_WINDOW: &str = "main";
const CLI_WINDOW: &str = "cli";

pub type Job = Box<dyn FnOnce(&mut dyn FnMut(&str, Value) -> Result<Value, Value>) -> Result<Value, Value> + Send>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub cmd: String,
    pub args: Value,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl From<Result<Value, Value>> for Reply {
    fn from(result: Result<Value, Value>) -> Self {
        match result {
            Ok(value) => Reply { ok: Some(value), error: None },
            Err(error) => Reply { ok: None, error: Some(error) },
        }
    }
}

impl Reply {
    fn into_result(self) -> Result<Value, Value> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.ok.unwrap_or(Value::Null)),
        }
    }
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(IDENTIFIER)
}

pub fn socket_path() -> PathBuf {
    data_dir().join(SOCKET_NAME)
}

fn app_url(app: &AppHandle) -> tauri::Url {
    let dev = if tauri::is_dev() { app.config().build.dev_url.clone() } else { None };
    dev.unwrap_or_else(|| "tauri://localhost".parse().expect("static url"))
}

pub async fn invoke(webview: Webview<Wry>, cmd: String, args: Value) -> Result<Value, Value> {
    let app = webview.app_handle().clone();
    let request = InvokeRequest {
        cmd,
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: app_url(&app),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: app.invoke_key().to_string(),
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        webview.on_message(request, Box::new(move |_, _, response, _, _| {
            let _ = tx.send(response);
        }));
    })
    .map_err(|e| json!(format!("The engine could not run the command: {}", e)))?;
    match rx.await {
        Ok(InvokeResponse::Ok(InvokeResponseBody::Json(text))) => Ok(serde_json::from_str(&text).unwrap_or(Value::String(text))),
        Ok(InvokeResponse::Ok(InvokeResponseBody::Raw(bytes))) => Ok(json!({ "bytes": bytes.len() })),
        Ok(InvokeResponse::Err(error)) => Err(error.0),
        Err(_) => Err(json!("The engine dropped the command without an answer")),
    }
}

fn is_locked_error(error: &Value) -> bool {
    error.as_str().map(|s| s.contains(LOCKED)).unwrap_or(false)
}

async fn invoke_unlocked(webview: Webview<Wry>, cmd: String, args: Value) -> Result<Value, Value> {
    let first = invoke(webview.clone(), cmd.clone(), args.clone()).await;
    match first {
        Err(error) if is_locked_error(&error) && cmd != "delete_lock" => {
            invoke(webview.clone(), "delete_lock".into(), json!({})).await.map_err(|e| json!(format!("Chatnip stayed locked: {}", e.as_str().unwrap_or("Touch ID was refused"))))?;
            invoke(webview, cmd, args).await
        }
        other => other,
    }
}

pub fn serve_socket(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let path = match handle.path().app_data_dir() {
            Ok(dir) => dir.join(SOCKET_NAME),
            Err(_) => return,
        };
        if tokio::net::UnixStream::connect(&path).await.is_ok() {
            eprintln!("Another Chatnip already answers on {}", path.display());
            return;
        }
        let _ = std::fs::remove_file(&path);
        let listener = match tokio::net::UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("The command line socket could not open: {}", e);
                return;
            }
        };
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        loop {
            let Ok((stream, _)) = listener.accept().await else { continue };
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
                let (read, mut write) = stream.into_split();
                let mut lines = tokio::io::BufReader::new(read).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let reply: Reply = match serde_json::from_str::<Request>(&line) {
                        Ok(request) => match handle.get_webview_window(MAIN_WINDOW) {
                            Some(window) => invoke(window.as_ref().clone(), request.cmd, request.args).await.into(),
                            None => Err(json!("The Chatnip window is not open")).into(),
                        },
                        Err(e) => Err(json!(format!("Unreadable request: {}", e))).into(),
                    };
                    let mut body = serde_json::to_vec(&reply).unwrap_or_default();
                    body.push(b'\n');
                    if write.write_all(&body).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
}

pub fn remove_socket(handle: &AppHandle) {
    if let Ok(dir) = handle.path().app_data_dir() {
        let _ = std::fs::remove_file(dir.join(SOCKET_NAME));
    }
}

fn ask(stream: &mut std::os::unix::net::UnixStream, cmd: &str, args: &Value) -> Result<Value, Value> {
    let mut body = serde_json::to_vec(&Request { cmd: cmd.to_string(), args: args.clone() }).map_err(|e| json!(e.to_string()))?;
    body.push(b'\n');
    stream.write_all(&body).map_err(|e| json!(format!("The running app did not take the command: {}", e)))?;
    let mut line = String::new();
    BufReader::new(stream.try_clone().map_err(|e| json!(e.to_string()))?).read_line(&mut line).map_err(|e| json!(e.to_string()))?;
    serde_json::from_str::<Reply>(&line).map_err(|e| json!(format!("The running app answered with something unreadable: {}", e)))?.into_result()
}

fn via_socket(cmd: &str, args: &Value) -> Option<Result<Value, Value>> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path()).ok()?;
    let first = ask(&mut stream, cmd, args);
    Some(match first {
        Err(error) if is_locked_error(&error) && cmd != "delete_lock" => match ask(&mut stream, "delete_lock", &json!({})) {
            Ok(_) => ask(&mut stream, cmd, args),
            Err(e) => Err(json!(format!("Chatnip stayed locked: {}", e.as_str().unwrap_or("Touch ID was refused")))),
        },
        other => other,
    })
}

fn headless(job: Job) -> Result<Value, Value> {
    let slot: Arc<Mutex<Option<Result<Value, Value>>>> = Arc::new(Mutex::new(None));
    let answer = slot.clone();
    let mut context = crate::app_context();
    context.config_mut().app.windows.clear();
    let app = crate::app_builder()
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Prohibited);
            crate::start_blocking(app.handle().clone(), false)?;
            let window = tauri::WebviewWindowBuilder::new(app, CLI_WINDOW, tauri::WebviewUrl::External("about:blank".parse().expect("static url")))
                .visible(false)
                .build()?;
            let webview = window.as_ref().clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let outcome = tauri::async_runtime::spawn_blocking(move || job(&mut |cmd, args| tauri::async_runtime::block_on(invoke_unlocked(webview.clone(), cmd.to_string(), args))))
                    .await
                    .unwrap_or_else(|e| Err(json!(format!("The engine stopped: {}", e))));
                *answer.lock().expect("answer slot") = Some(outcome);
                handle.exit(0);
            });
            Ok(())
        })
        .build(context)
        .map_err(|e| json!(format!("The engine did not start: {}", e)))?;
    app.run_return(|_, _| {});
    let outcome = slot.lock().expect("answer slot").take();
    outcome.unwrap_or_else(|| Err(json!("The engine stopped without an answer")))
}

pub fn dispatch(job: Job) -> Result<Value, Value> {
    if running() {
        return job(&mut |cmd, args| via_socket(cmd, &args).unwrap_or_else(|| Err(json!("The app closed while the command ran"))));
    }
    headless(job)
}

pub fn running() -> bool {
    std::os::unix::net::UnixStream::connect(socket_path()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identifier_matches_the_app_config() {
        let config: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(config["identifier"], IDENTIFIER);
        assert!(socket_path().ends_with(format!("{}/{}", IDENTIFIER, SOCKET_NAME)));
    }

    #[test]
    fn replies_round_trip_results_and_errors() {
        let ok: Reply = Ok(json!({ "a": 1 })).into();
        let text = serde_json::to_string(&ok).unwrap();
        assert_eq!(serde_json::from_str::<Reply>(&text).unwrap().into_result(), Ok(json!({ "a": 1 })));
        let err: Reply = Err(json!(LOCKED)).into();
        let back = serde_json::from_str::<Reply>(&serde_json::to_string(&err).unwrap()).unwrap().into_result().unwrap_err();
        assert!(is_locked_error(&back));
    }
}
