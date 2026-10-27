use fantoccini::{elements::Element, Client, ClientBuilder, Locator};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;

pub const BINARY: &str = env!("CARGO_BIN_EXE_chatnip");
pub const IDENTIFIER: &str = "com.slyotis.chatnip";
pub const SLOTS: usize = 4;
pub const WAIT: Duration = Duration::from_secs(20);
pub const POLL: Duration = Duration::from_millis(100);
pub const MOCK_KINDS: [&str; 6] = ["teams", "outlook", "slack", "gmail", "google_chat", "imap"];

static GOLDEN: OnceCell<PathBuf> = OnceCell::const_new();
static GOLDEN_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static SWEEP: std::sync::Once = std::sync::Once::new();

extern "C" fn drop_golden() {
    if let Some(path) = GOLDEN_PATH.get() {
        let _ = std::fs::remove_dir_all(path);
    }
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
}

fn sweep() {
    SWEEP.call_once(|| {
        for pid in output("/usr/bin/pgrep", &["-f", BINARY]).split_whitespace() {
            let parent = output("/bin/ps", &["-o", "ppid=", "-p", pid]);
            let command = output("/bin/ps", &["eww", "-o", "command=", "-p", pid]);
            let home = command.split_whitespace().find_map(|part| part.strip_prefix("CHATNIP_HOME=")).unwrap_or_default();
            let orphan = parent.trim() == "1";
            if orphan && (home.starts_with("/tmp/cn") || home.starts_with("/private/tmp/cn")) {
                if let Ok(pid) = pid.parse::<i32>() {
                    eprintln!("stopped a leftover E2E instance {} on {}", pid, home);
                    unsafe { libc::kill(pid, libc::SIGKILL) };
                }
            }
        }
    });
}

pub fn tid(name: &str) -> String {
    format!("[data-testid=\"{}\"]", name)
}

pub fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

pub const PORT_BASE: u16 = 47610;

fn wait_port(port: u16) {
    let started = Instant::now();
    while std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
        assert!(started.elapsed() < Duration::from_secs(15), "port {} stays taken by another process, so this slot cannot start an instance", port);
        std::thread::sleep(Duration::from_millis(200));
    }
}

pub struct Slot {
    _file: File,
    index: u16,
}

impl Slot {
    fn webdriver(&self) -> u16 {
        PORT_BASE + self.index * 2
    }

    fn oauth(&self) -> u16 {
        PORT_BASE + self.index * 2 + 1
    }
}

fn data_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support").join(IDENTIFIER)
}

fn slot() -> Slot {
    sweep();
    let dir = std::env::temp_dir().join("chatnip-e2e-slots");
    std::fs::create_dir_all(&dir).unwrap();
    loop {
        for index in 0..SLOTS {
            let file = File::create(dir.join(format!("{}.lock", index))).unwrap();
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Slot { _file: file, index: index as u16 };
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_tree(&path, &target);
        } else if kind.is_file() && path.extension().and_then(|e| e.to_str()) != Some("sock") && entry.file_name() != "app.log" {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

pub struct App {
    pub client: Client,
    pub home: PathBuf,
    pub port: u16,
    child: Child,
    keep: Option<tempfile::TempDir>,
    slot: Slot,
    debug: Option<&'static str>,
}

impl App {
    pub async fn fresh() -> App {
        let golden = GOLDEN.get_or_init(|| async { Self::golden().await }).await.clone();
        let dir = tempfile::Builder::new().prefix("cn").tempdir_in("/tmp").unwrap();
        copy_tree(&golden, dir.path());
        let app = Self::start(dir, slot(), Some("true")).await;
        app.ready().await;
        app.guard().await;
        app
    }

    pub async fn guard(&self) {
        assert!(self.home.starts_with("/tmp") || self.home.starts_with("/private/tmp"), "the E2E home {} is not a fresh /tmp home", self.home.display());
        for provider in self.providers().await {
            assert_eq!(provider["provider_type"].as_str(), Some("mock"), "the E2E instance sees a non mock connection: {}", provider);
        }
    }

    pub async fn fresh_unready() -> App {
        let golden = GOLDEN.get_or_init(|| async { Self::golden().await }).await.clone();
        let dir = tempfile::Builder::new().prefix("cn").tempdir_in("/tmp").unwrap();
        copy_tree(&golden, dir.path());
        Self::start(dir, slot(), Some("true")).await
    }

    pub async fn blank(debug: Option<&'static str>) -> App {
        let dir = tempfile::Builder::new().prefix("cn").tempdir_in("/tmp").unwrap();
        Self::start(dir, slot(), debug).await
    }

    async fn golden() -> PathBuf {
        let dir = tempfile::Builder::new().prefix("cng").tempdir_in("/tmp").unwrap();
        let mut app = Self::start(dir, slot(), Some("true")).await;
        app.ready().await;
        app.wait_url("/main").await;
        for (key, value) in [("assistant.enabled", "false"), ("ollama.host", "http://127.0.0.1:9"), ("app.locale", "en"), ("mock.reply_seconds", "3"), ("links.previews", "false")] {
            app.engine("put_app_setting", json!({ "key": key, "value": value })).await.unwrap();
        }
        let mut kinds = serde_json::Map::new();
        for provider in app.providers().await {
            if let Some(kind) = provider["namespace"].as_str() {
                kinds.insert(kind.to_string(), provider["id"].clone());
            }
        }
        assert_eq!(kinds.len(), MOCK_KINDS.len(), "the debug start did not connect every mock client: {:?}", kinds);
        std::fs::write(app.home.join("e2e-providers.json"), Value::Object(kinds).to_string()).unwrap();
        let started = Instant::now();
        while app.providers().await.iter().any(|p| p["last_sync_at"].is_null()) || app.conversations().await.len() < 10 {
            assert!(started.elapsed() < Duration::from_secs(90), "the mock clients never synced before the seed; log {}", app.home.join("app.log").display());
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        app.relaunch().await;
        app.wait_log("Startup tasks finished").await;
        app.ready().await;
        app.guard().await;
        let started = Instant::now();
        while app.conversations().await.len() < 10 {
            assert!(started.elapsed() < Duration::from_secs(90), "the mock conversations never synced; log {}", app.home.join("app.log").display());
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        app.stop();
        let path = app.keep.take().unwrap().keep();
        if GOLDEN_PATH.set(path.clone()).is_ok() {
            unsafe { libc::atexit(drop_golden) };
        }
        path
    }

    async fn start(dir: tempfile::TempDir, slot: Slot, debug: Option<&'static str>) -> App {
        let home = dir.path().to_path_buf();
        let port = slot.webdriver();
        wait_port(port);
        wait_port(slot.oauth());
        let mut child = Self::spawn(&home, port, slot.oauth(), debug);
        match Self::connect(port).await {
            Some(client) => App { client, home, port, child, keep: Some(dir), slot, debug },
            None => {
                let _ = child.kill();
                let kept = dir.keep();
                panic!("WebDriver on port {} did not answer; log in {}", port, kept.join("app.log").display());
            }
        }
    }

    fn spawn(home: &Path, port: u16, oauth: u16, debug: Option<&str>) -> Child {
        let log = File::options().create(true).append(true).open(home.join("app.log")).unwrap();
        let mut command = Command::new(BINARY);
        command.current_dir(home).env_clear();
        if let Some(debug) = debug {
            command.env("CHATNIP_USE_DEBUG", debug);
        }
        command
            .env("PATH", format!("{}/tests/fixtures/bin:/usr/bin:/bin:/usr/sbin:/sbin", env!("CARGO_MANIFEST_DIR")))
            .env("CHATNIP_HOME", home)
            .env("CHATNIP_SECRETS", "memory")
            .env("CHATNIP_OAUTH_PORT", oauth.to_string())
            .env("TAURI_WEBDRIVER_PORT", port.to_string())
            .env("TMPDIR", home)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("the chatnip binary starts")
    }

    async fn connect(port: u16) -> Option<Client> {
        let started = Instant::now();
        loop {
            if let Ok(client) = ClientBuilder::native().connect(&format!("http://127.0.0.1:{}", port)).await {
                return Some(client);
            }
            if started.elapsed() > Duration::from_secs(60) {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(data_dir(&self.home).join("chatnip.sock"));
    }

    pub async fn relaunch(&mut self) {
        self.stop();
        wait_port(self.port);
        wait_port(self.slot.oauth());
        self.child = Self::spawn(&self.home, self.port, self.slot.oauth(), self.debug);
        self.client = Self::connect(self.port).await.unwrap_or_else(|| panic!("WebDriver on port {} did not answer after the relaunch; log in {}", self.port, self.home.join("app.log").display()));
    }

    pub async fn relaunch_ready(&mut self) {
        self.relaunch().await;
        self.ready().await;
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.home.join("app.log")).unwrap_or_default()
    }

    pub async fn wait_log(&self, needle: &str) {
        let started = Instant::now();
        while !self.log().contains(needle) {
            if started.elapsed() > Duration::from_secs(60) {
                panic!("the app log never said {:?}; log in {}", needle, self.home.join("app.log").display());
            }
            tokio::time::sleep(POLL).await;
        }
    }

    pub async fn ready(&self) {
        self.wait_for("the app shell is ready", &format!("document.querySelector('{}[data-ready=\"true\"]') && document.querySelector('[data-router]')", tid("app-root")), Duration::from_secs(60)).await;
    }

    pub fn socket(&self) -> PathBuf {
        data_dir(&self.home).join("chatnip.sock")
    }

    pub async fn engine(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let path = self.socket();
        let request = json!({ "cmd": cmd, "args": args });
        let started = Instant::now();
        loop {
            let socket = path.clone();
            let body = request.clone();
            let answer = tokio::task::spawn_blocking(move || -> Option<Value> {
                let mut stream = UnixStream::connect(&socket).ok()?;
                let mut line = serde_json::to_vec(&body).ok()?;
                line.push(b'\n');
                stream.write_all(&line).ok()?;
                let mut reply = String::new();
                BufReader::new(stream).read_line(&mut reply).ok()?;
                serde_json::from_str(&reply).ok()
            })
            .await
            .unwrap();
            match answer {
                Some(reply) => {
                    return match reply.get("error") {
                        Some(error) => Err(error.clone()),
                        None => Ok(reply.get("ok").cloned().unwrap_or(Value::Null)),
                    }
                }
                None if started.elapsed() < WAIT => tokio::time::sleep(Duration::from_millis(200)).await,
                None => panic!("the engine socket {} did not answer {}", path.display(), cmd),
            }
        }
    }

    pub async fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        let script = "const done = arguments[arguments.length - 1]; window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]).then((ok) => done({ ok }), (error) => done({ error: String(error) }))";
        let reply = self.client.execute_async(script, vec![json!(cmd), args]).await.unwrap();
        match reply.get("error") {
            Some(error) => Err(error.clone()),
            None => Ok(reply.get("ok").cloned().unwrap_or(Value::Null)),
        }
    }

    pub async fn js(&self, script: &str) -> Value {
        self.client.execute(script, vec![]).await.unwrap_or_else(|e| panic!("script failed: {}\n{}", e, script))
    }

    pub async fn js_with(&self, script: &str, args: Vec<Value>) -> Value {
        self.client.execute(script, args).await.unwrap_or_else(|e| panic!("script failed: {}\n{}", e, script))
    }

    pub async fn check(&self, expression: &str) -> bool {
        let script = format!("try {{ return !!({}) }} catch (e) {{ return false }}", expression);
        self.client.execute(&script, vec![]).await.map(|v| v.as_bool().unwrap_or(false)).unwrap_or(false)
    }

    pub async fn wait(&self, what: &str, expression: &str) {
        self.wait_for(what, expression, WAIT).await
    }

    pub async fn wait_for(&self, what: &str, expression: &str, timeout: Duration) {
        let started = Instant::now();
        while !self.check(expression).await {
            if started.elapsed() > timeout {
                panic!("timed out waiting for {} ({}); url {}; log {}", what, expression, self.url().await, self.home.join("app.log").display());
            }
            tokio::time::sleep(POLL).await;
        }
    }

    pub async fn url(&self) -> String {
        self.client.current_url().await.map(|u| u.to_string()).unwrap_or_default()
    }

    pub async fn wait_url(&self, fragment: &str) {
        self.wait(&format!("the url to contain {}", fragment), &format!("location.href.includes({})", js_string(fragment))).await
    }

    pub async fn goto(&self, path: &str) {
        self.js("window.__e2eStale = true").await;
        self.client.goto(&format!("tauri://localhost{}", path)).await.unwrap();
        self.wait("the old page to unload", "!window.__e2eStale").await;
        self.ready().await;
        self.wait_url(path.split('?').next().unwrap_or(path)).await;
    }

    pub async fn exists(&self, selector: &str) -> bool {
        self.check(&format!("document.querySelector({})", js_string(selector))).await
    }

    pub async fn count(&self, selector: &str) -> usize {
        self.js(&format!("return document.querySelectorAll({}).length", js_string(selector))).await.as_u64().unwrap_or(0) as usize
    }

    pub async fn visible(&self, selector: &str) -> bool {
        self.check(&format!("(() => {{ const el = document.querySelector({}); if (!el) return false; const box = el.getBoundingClientRect(); return box.width > 0 && box.height > 0 && getComputedStyle(el).visibility !== 'hidden' }})()", js_string(selector))).await
    }

    pub async fn see(&self, selector: &str) {
        self.wait(&format!("{} to exist", selector), &format!("document.querySelector({})", js_string(selector))).await
    }

    pub async fn gone(&self, selector: &str) {
        self.wait(&format!("{} to go away", selector), &format!("!document.querySelector({})", js_string(selector))).await
    }

    pub async fn find(&self, selector: &str) -> Element {
        self.see(selector).await;
        self.client.find(Locator::Css(selector)).await.unwrap_or_else(|e| panic!("{} not found: {}", selector, e))
    }

    pub async fn click(&self, selector: &str) {
        self.find(selector).await.click().await.unwrap_or_else(|e| panic!("click on {} failed: {}", selector, e));
    }

    pub async fn select(&self, trigger: &str, value: &str) {
        self.click(trigger).await;
        self.see("[data-testid=\"select-dialog\"]").await;
        self.click(&format!("[data-testid=\"select-option\"][data-value={}]", js_string(value))).await;
        self.gone("[data-testid=\"select-dialog\"]").await;
        self.wait(&format!("{} to show {}", trigger, value), &format!("document.querySelector({})?.dataset.value === {}", js_string(trigger), js_string(value))).await;
    }

    pub async fn type_into(&self, selector: &str, text: &str) {
        self.find(selector).await.send_keys(text).await.unwrap_or_else(|e| panic!("typing into {} failed: {}", selector, e));
    }

    pub async fn fill(&self, selector: &str, text: &str) {
        let field = self.find(selector).await;
        field.clear().await.unwrap();
        field.send_keys(text).await.unwrap();
    }

    pub async fn text(&self, selector: &str) -> String {
        self.see(selector).await;
        self.js(&format!("return document.querySelector({}).textContent.trim()", js_string(selector))).await.as_str().unwrap_or_default().to_string()
    }

    pub async fn texts(&self, selector: &str) -> Vec<String> {
        self.js(&format!("return [...document.querySelectorAll({})].map((el) => el.textContent.trim())", js_string(selector))).await.as_array().cloned().unwrap_or_default().into_iter().filter_map(|v| v.as_str().map(String::from)).collect()
    }

    pub async fn attr(&self, selector: &str, name: &str) -> Option<String> {
        self.see(selector).await;
        self.js(&format!("return document.querySelector({}).getAttribute({})", js_string(selector), js_string(name))).await.as_str().map(String::from)
    }

    pub async fn value(&self, selector: &str) -> String {
        self.see(selector).await;
        self.js(&format!("const el = document.querySelector({}); return el.isContentEditable ? el.innerText : el.value", js_string(selector))).await.as_str().unwrap_or_default().to_string()
    }

    pub async fn press(&self, selector: Option<&str>, key: &str, modifiers: &[&str]) {
        let target = match selector {
            Some(selector) => format!("document.querySelector({})", js_string(selector)),
            None => "(document.activeElement || document.body)".to_string(),
        };
        let script = format!(
            "const el = {}; const init = {{ key: {}, bubbles: true, cancelable: true, metaKey: {}, shiftKey: {}, altKey: {}, ctrlKey: {} }}; const down = new KeyboardEvent('keydown', init); const handled = !el.dispatchEvent(down); el.dispatchEvent(new KeyboardEvent('keyup', init)); return handled",
            target,
            js_string(key),
            modifiers.contains(&"meta"),
            modifiers.contains(&"shift"),
            modifiers.contains(&"alt"),
            modifiers.contains(&"ctrl")
        );
        self.js(&script).await;
    }

    pub async fn hover(&self, selector: &str) {
        self.see(selector).await;
        self.js(&format!("const el = document.querySelector({}); for (const type of ['pointerover', 'pointerenter', 'mouseover', 'mouseenter', 'pointermove', 'mousemove']) el.dispatchEvent(new PointerEvent(type, {{ bubbles: true }}))", js_string(selector))).await;
    }

    pub async fn pointer_click(&self, selector: &str) {
        self.see(selector).await;
        self.js(&format!("const el = document.querySelector({}); el.scrollIntoView({{ block: 'center' }}); const box = el.getBoundingClientRect(); const init = {{ bubbles: true, cancelable: true, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2, button: 0, buttons: 1, pointerId: 1, isPrimary: true }}; el.dispatchEvent(new PointerEvent('pointerdown', init)); el.dispatchEvent(new MouseEvent('mousedown', init)); el.dispatchEvent(new PointerEvent('pointerup', {{ ...init, buttons: 0 }})); el.dispatchEvent(new MouseEvent('mouseup', {{ ...init, buttons: 0 }})); el.click()", js_string(selector))).await;
    }

    pub async fn context_click(&self, selector: &str) {
        self.see(selector).await;
        self.js(&format!("const el = document.querySelector({}); el.scrollIntoView({{ block: 'center' }}); const box = el.getBoundingClientRect(); el.dispatchEvent(new MouseEvent('contextmenu', {{ bubbles: true, cancelable: true, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2, button: 2 }}))", js_string(selector))).await;
    }

    pub async fn confirm_next(&self, answer: bool) {
        self.js(&format!("window.confirm = () => {}", answer)).await;
    }

    pub async fn prompt_next(&self, answer: &str) {
        self.js(&format!("window.prompt = () => {}", js_string(answer))).await;
    }

    pub async fn setting(&self, key: &str) -> Option<String> {
        let settings = self.engine("get_app_settings", json!({})).await.unwrap();
        settings.get(key).and_then(|v| v.as_str()).map(String::from)
    }

    pub async fn put_setting(&self, key: &str, value: &str) {
        self.engine("put_app_setting", json!({ "key": key, "value": value })).await.unwrap();
    }

    pub async fn conversations(&self) -> Vec<Value> {
        self.engine("get_conversations", json!({})).await.unwrap()["data"].as_array().cloned().unwrap_or_default()
    }

    pub async fn providers(&self) -> Vec<Value> {
        self.engine("get_providers", json!({})).await.unwrap()["data"].as_array().cloned().unwrap_or_default()
    }

    pub fn mock_provider(&self, kind: &str) -> String {
        let kinds: Value = serde_json::from_str(&std::fs::read_to_string(self.home.join("e2e-providers.json")).unwrap()).unwrap();
        kinds[kind].as_str().unwrap_or_else(|| panic!("no mock provider of kind {}", kind)).to_string()
    }

    pub async fn intercept(&self, commands: &[&str]) {
        let list = serde_json::to_string(commands).unwrap();
        let script = r#"
            window.__e2eBlocked = new Set(BLOCKED);
            window.__e2eCalls = window.__e2eCalls || [];
            if (!window.__e2eFetch) {
                window.__e2eFetch = window.fetch;
                window.fetch = (input, init) => {
                    const url = typeof input === 'string' ? input : (input && input.url) || '';
                    const prefix = 'ipc://localhost/';
                    const cmd = url.startsWith(prefix) ? decodeURIComponent(url.slice(prefix.length).split('?')[0]) : null;
                    if (cmd && window.__e2eBlocked.has(cmd)) {
                        let args = null;
                        try { args = JSON.parse(init && init.body) } catch (e) {}
                        window.__e2eCalls.push({ cmd, args });
                        return Promise.resolve(new Response('null', { headers: { 'Tauri-Response': 'ok', 'content-type': 'application/json' } }));
                    }
                    return window.__e2eFetch(input, init);
                };
            }
        "#;
        self.js(&script.replace("BLOCKED", &list)).await;
    }

    pub async fn intercepted(&self, command: &str) -> Vec<Value> {
        self.js_with("return (window.__e2eCalls || []).filter((c) => c.cmd === arguments[0]).map((c) => c.args ?? null)", vec![json!(command)]).await.as_array().cloned().unwrap_or_default()
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.stop();
        if std::thread::panicking() {
            if let Some(dir) = self.keep.take() {
                eprintln!("kept the failing profile at {}", dir.keep().display());
            }
        }
    }
}
