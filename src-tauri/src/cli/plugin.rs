use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const NAME: &str = "chatnip";
pub const SHORT: &str = "cnip";
pub const PLUGIN_ID: &str = "chatnip@chatnip";
pub const SKILL: &str = include_str!("../../skill/chatnip/SKILL.md");
const CONFIG_DIR: &str = ".chatnip";
const PLUGIN_DIR: &str = "plugin";
const LINK_DIR: &str = ".local/bin";
const MANIFEST: &str = ".claude-plugin/plugin.json";
const HOME_ENV: &str = "CHATNIP_HOME";
const MOCK_ENV: &str = "CHATNIP_USE_DEBUG";
const DESCRIPTION: &str = "Read and work every Chatnip inbox from any repo: the chatnip skill and an mcp server exposing every Chatnip command as a tool.";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginOptions {
    #[serde(default)]
    pub home: Option<String>,
    #[serde(default)]
    pub mock: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliSetup {
    pub executable: String,
    pub link: String,
    pub linked: bool,
    pub on_path: bool,
    pub claude: Option<String>,
    pub plugin_dir: String,
    pub plugin_version: Option<String>,
    pub plugin_current: String,
    pub plugin_installed: bool,
    pub plugin_options: Option<PluginOptions>,
}

pub fn home() -> PathBuf {
    let account = unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_dir.is_null() {
            None
        } else {
            std::ffi::CStr::from_ptr((*entry).pw_dir).to_str().ok().map(PathBuf::from)
        }
    };
    account.or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir)
}

pub fn plugin_dir() -> PathBuf {
    home().join(CONFIG_DIR).join(PLUGIN_DIR)
}

pub fn link_dir() -> PathBuf {
    home().join(LINK_DIR)
}

pub fn executable() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("The running binary could not be found: {}", e))?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

pub fn expand_home(path: &str) -> String {
    let path = path.trim();
    match path.strip_prefix('~') {
        Some("") => home().to_string_lossy().to_string(),
        Some(rest) if rest.starts_with('/') => home().join(rest.trim_start_matches('/')).to_string_lossy().to_string(),
        _ => path.to_string(),
    }
}

impl PluginOptions {
    pub fn resolved(&self) -> PluginOptions {
        let home = self.home.as_deref().map(expand_home).filter(|home| !home.is_empty()).map(|home| match Path::new(&home).is_absolute() {
            true => home,
            false => std::env::current_dir().unwrap_or_else(|_| self::home()).join(&home).to_string_lossy().to_string(),
        });
        PluginOptions { home, mock: self.mock }
    }

    pub fn get_installed(manifest: &Value) -> PluginOptions {
        let env = &manifest["mcpServers"][NAME]["env"];
        PluginOptions {
            home: env[HOME_ENV].as_str().filter(|home| !home.is_empty()).map(String::from),
            mock: env[MOCK_ENV].as_str().map(|value| value.trim().eq_ignore_ascii_case("true")).unwrap_or(false),
        }
    }
}

pub fn get_written(dir: &Path) -> Option<Value> {
    std::fs::read_to_string(dir.join(MANIFEST)).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok())
}

pub fn manifest(executable: &str, options: &PluginOptions) -> Value {
    let mut server = json!({ "command": executable, "args": ["mcp"] });
    let mut env = serde_json::Map::new();
    if let Some(home) = options.home.as_deref().filter(|home| !home.is_empty()) {
        env.insert(HOME_ENV.into(), json!(home));
    }
    if options.mock {
        env.insert(MOCK_ENV.into(), json!("true"));
    }
    if !env.is_empty() {
        server["env"] = Value::Object(env);
    }
    let mut manifest = json!({
        "name": NAME,
        "description": DESCRIPTION,
        "author": { "name": "slyotis" },
        "skills": "./skills/",
        "mcpServers": { NAME: server }
    });
    let hash = crate::utils::content_hash(&[manifest.to_string().as_bytes(), SKILL.as_bytes()].concat());
    manifest["version"] = json!(format!("{}-{}", env!("CARGO_PKG_VERSION"), &hash[..12]));
    manifest
}

fn write(dir: &Path, name: &str, body: &[u8]) -> Result<(), String> {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} could not be created: {}", parent.display(), e))?;
    }
    std::fs::write(&path, body).map_err(|e| format!("{} could not be written: {}", path.display(), e))
}

pub fn write_plugin(dir: &Path, executable: &str, options: &PluginOptions) -> Result<String, String> {
    let manifest = manifest(executable, options);
    let version = manifest["version"].as_str().unwrap_or_default().to_string();
    let marketplace = json!({
        "name": NAME,
        "owner": { "name": "slyotis" },
        "description": "The chatnip plugin this chatnip binary carries.",
        "plugins": [{ "name": NAME, "source": "./", "description": DESCRIPTION, "version": version }]
    });
    write(dir, MANIFEST, format!("{:#}\n", manifest).as_bytes())?;
    write(dir, ".claude-plugin/marketplace.json", format!("{:#}\n", marketplace).as_bytes())?;
    write(dir, &format!("skills/{}/SKILL.md", NAME), SKILL.as_bytes())?;
    Ok(version)
}

pub fn put_skill(base: &Path) -> Result<PathBuf, String> {
    let dest = base.join("skills").join(NAME);
    write(&dest, "SKILL.md", SKILL.as_bytes())?;
    Ok(dest)
}

pub fn claude() -> Option<PathBuf> {
    crate::services::intelligence_service::resolve_binary("claude")
}

fn run_claude(claude: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(claude).args(args).output().map_err(|e| format!("claude {} could not start: {}", args.join(" "), e))?;
    if !output.status.success() {
        return Err(format!("claude {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn listed_names(json_text: &str, key: &str) -> Vec<String> {
    serde_json::from_str::<Vec<Value>>(json_text)
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item.get(key).and_then(Value::as_str).map(String::from))
        .collect()
}

pub fn install_plugin(project: bool, options: &PluginOptions) -> Result<Vec<String>, String> {
    let claude = claude().ok_or_else(|| "The claude cli is not installed; the plugin installs through it".to_string())?;
    let exe = executable()?;
    let dir = plugin_dir();
    let options = options.resolved();
    let version = write_plugin(&dir, &exe.to_string_lossy(), &options)?;
    let mut steps = vec![format!("wrote {} version {}, mcp {} mcp", dir.display(), version, exe.display())];
    steps.push(format!("mcp home {}, mock mode {}", options.home.as_deref().unwrap_or("app data"), if options.mock { "forced on" } else { "from settings" }));
    let scope = if project { "project" } else { "user" };
    let dir_text = dir.to_string_lossy().to_string();
    let marketplaces = listed_names(&run_claude(&claude, &["plugin", "marketplace", "list", "--json"])?, "name");
    if marketplaces.iter().any(|m| m == NAME) {
        run_claude(&claude, &["plugin", "marketplace", "update", NAME])?;
        steps.push("updated the chatnip marketplace".into());
    } else {
        run_claude(&claude, &["plugin", "marketplace", "add", &dir_text, "--scope", scope])?;
        steps.push(format!("added the chatnip marketplace for the {}", scope));
    }
    let installed = listed_names(&run_claude(&claude, &["plugin", "list", "--json"])?, "id");
    if installed.iter().any(|p| p == PLUGIN_ID) {
        run_claude(&claude, &["plugin", "update", PLUGIN_ID])?;
        steps.push(format!("updated {}", PLUGIN_ID));
    } else {
        run_claude(&claude, &["plugin", "install", PLUGIN_ID, "--scope", scope])?;
        steps.push(format!("installed {} for the {}", PLUGIN_ID, scope));
    }
    Ok(steps)
}

pub fn link(dir: Option<PathBuf>) -> Result<PathBuf, String> {
    let dir = dir.unwrap_or_else(link_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{} could not be created: {}", dir.display(), e))?;
    let exe = executable()?;
    for name in [NAME, SHORT] {
        let target = dir.join(name);
        if let Ok(meta) = std::fs::symlink_metadata(&target) {
            if !meta.file_type().is_symlink() {
                return Err(format!("{} exists and is not a link; move it away first", target.display()));
            }
            std::fs::remove_file(&target).map_err(|e| format!("The old link could not be replaced: {}", e))?;
        }
        std::os::unix::fs::symlink(&exe, &target).map_err(|e| format!("{} could not be linked: {}", target.display(), e))?;
    }
    Ok(dir.join(NAME))
}

pub fn on_path(dir: &Path) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|entry| entry == dir)).unwrap_or(false)
}

pub fn setup() -> CliSetup {
    let exe = executable().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let target = link_dir().join(NAME);
    let linked = std::fs::read_link(&target).map(|p| p.to_string_lossy() == exe).unwrap_or(false);
    let claude = claude();
    let written = get_written(&plugin_dir());
    let plugin_version = written.as_ref().and_then(|v| v["version"].as_str().map(String::from));
    let plugin_options = written.as_ref().map(PluginOptions::get_installed);
    let plugin_installed = claude
        .as_ref()
        .and_then(|c| run_claude(c, &["plugin", "list", "--json"]).ok())
        .map(|text| listed_names(&text, "id").iter().any(|p| p == PLUGIN_ID))
        .unwrap_or(false);
    CliSetup {
        executable: exe.clone(),
        link: target.to_string_lossy().to_string(),
        linked,
        on_path: on_path(&link_dir()),
        claude: claude.map(|p| p.to_string_lossy().to_string()),
        plugin_dir: plugin_dir().to_string_lossy().to_string(),
        plugin_version,
        plugin_current: manifest(&exe, &plugin_options.clone().unwrap_or_default())["version"].as_str().unwrap_or_default().to_string(),
        plugin_installed,
        plugin_options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plugin_carries_the_skill_and_the_mcp_server() {
        let dir = tempfile::tempdir().unwrap();
        let version = write_plugin(dir.path(), "/opt/chatnip", &PluginOptions::default()).unwrap();
        let written: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude-plugin/plugin.json")).unwrap()).unwrap();
        assert_eq!(written["name"], NAME);
        assert_eq!(written["version"], version);
        assert_eq!(written["mcpServers"][NAME]["command"], "/opt/chatnip");
        assert_eq!(written["mcpServers"][NAME]["args"], json!(["mcp"]));
        let marketplace: Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude-plugin/marketplace.json")).unwrap()).unwrap();
        assert_eq!(marketplace["plugins"][0]["version"], version);
        assert_eq!(std::fs::read_to_string(dir.path().join("skills/chatnip/SKILL.md")).unwrap(), SKILL);
        assert_ne!(manifest("/elsewhere", &PluginOptions::default())["version"], json!(version));
    }

    #[test]
    fn the_mcp_env_comes_only_from_the_chosen_options() {
        let plain = manifest("/opt/chatnip", &PluginOptions::default());
        assert!(plain["mcpServers"][NAME].get("env").is_none());
        let home = manifest("/opt/chatnip", &PluginOptions { home: Some("/tmp/cn-home".into()), mock: false });
        assert_eq!(home["mcpServers"][NAME]["env"], json!({ "CHATNIP_HOME": "/tmp/cn-home" }));
        let mock = manifest("/opt/chatnip", &PluginOptions { home: None, mock: true });
        assert_eq!(mock["mcpServers"][NAME]["env"], json!({ "CHATNIP_USE_DEBUG": "true" }));
        let both = manifest("/opt/chatnip", &PluginOptions { home: Some("/tmp/cn-home".into()), mock: true });
        assert_eq!(both["mcpServers"][NAME]["env"], json!({ "CHATNIP_HOME": "/tmp/cn-home", "CHATNIP_USE_DEBUG": "true" }));
        assert_ne!(plain["version"], both["version"]);
        assert!(manifest("/opt/chatnip", &PluginOptions { home: Some(String::new()), mock: false })["mcpServers"][NAME].get("env").is_none());
    }

    #[test]
    fn status_reads_the_installed_options_back() {
        let dir = tempfile::tempdir().unwrap();
        assert!(get_written(dir.path()).is_none());
        let chosen = PluginOptions { home: Some("/tmp/cn-home".into()), mock: true };
        write_plugin(dir.path(), "/opt/chatnip", &chosen).unwrap();
        assert_eq!(get_written(dir.path()).map(|m| PluginOptions::get_installed(&m)), Some(chosen));
        write_plugin(dir.path(), "/opt/chatnip", &PluginOptions::default()).unwrap();
        assert_eq!(get_written(dir.path()).map(|m| PluginOptions::get_installed(&m)), Some(PluginOptions::default()));
    }

    #[test]
    fn a_chosen_home_resolves_to_an_absolute_path() {
        let account = home().to_string_lossy().to_string();
        assert_eq!(expand_home("~/.chatnip-mock"), format!("{}/.chatnip-mock", account));
        assert_eq!(expand_home("~"), account);
        assert_eq!(expand_home("/tmp/x"), "/tmp/x");
        assert_eq!(expand_home("~other/x"), "~other/x");
        assert_eq!(PluginOptions { home: Some("  ".into()), mock: true }.resolved(), PluginOptions { home: None, mock: true });
        assert!(Path::new(PluginOptions { home: Some("relative".into()), mock: false }.resolved().home.as_deref().unwrap()).is_absolute());
    }

    #[test]
    fn the_skill_installs_under_a_chosen_claude_folder() {
        let dir = tempfile::tempdir().unwrap();
        let dest = put_skill(dir.path()).unwrap();
        assert!(dest.join("SKILL.md").exists());
        assert!(SKILL.starts_with("---\nname: chatnip"));
    }

    #[test]
    fn linking_refuses_to_replace_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(NAME), "x").unwrap();
        assert!(link(Some(dir.path().to_path_buf())).is_err());
        std::fs::remove_file(dir.path().join(NAME)).unwrap();
        let target = link(Some(dir.path().to_path_buf())).unwrap();
        assert!(std::fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
        assert!(std::fs::symlink_metadata(dir.path().join(SHORT)).unwrap().file_type().is_symlink());
        assert!(link(Some(dir.path().to_path_buf())).is_ok());
    }

    #[test]
    fn listed_names_read_claude_json() {
        assert_eq!(listed_names(r#"[{"id":"chatnip@chatnip","scope":"user"}]"#, "id"), vec!["chatnip@chatnip"]);
        assert!(listed_names("not json", "id").is_empty());
    }
}
