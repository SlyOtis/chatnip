use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FILE: &str = "config.yaml";
pub const LOCAL_DIR: &str = ".chatnip";
pub const CONFIG_ENV: &str = "CHATNIP_CONFIG";
pub const KEYS: [(&str, &str); 3] = [("home", "CHATNIP_HOME"), ("use_debug", "CHATNIP_USE_DEBUG"), ("secrets", "CHATNIP_SECRETS")];
const SECRET_MARKS: [&str; 5] = ["token", "secret", "password", "key", "credential"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub configs: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Local {
    #[serde(default, rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_config: Option<String>,
    #[serde(default, flatten)]
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Value {
    pub key: String,
    pub value: String,
    pub layer: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub name: Option<String>,
    pub named_by: Option<String>,
    pub values: Vec<Value>,
    pub local: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub file: PathBuf,
    pub key: String,
}

pub fn home_file() -> PathBuf {
    super::plugin::home().join(LOCAL_DIR).join(FILE)
}

pub fn read_file(path: &Path) -> Result<File, String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(File::default()),
        Ok(text) => serde_yaml_ng::from_str(&text).map_err(|e| format!("{} is not valid YAML: {}", path.display(), e)),
        Err(_) => Ok(File::default()),
    }
}

pub fn write_file(path: &Path, file: &File) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{} could not be created: {}", parent.display(), e))?;
    }
    let text = serde_yaml_ng::to_string(file).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("{} could not be written: {}", path.display(), e))?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    Ok(())
}

pub fn find_local(start: &Path) -> Option<PathBuf> {
    let home = super::plugin::home();
    let mut dir = Some(start);
    while let Some(current) = dir {
        let candidate = current.join(LOCAL_DIR).join(FILE);
        if candidate.is_file() && current != home {
            return Some(candidate);
        }
        dir = current.parent();
    }
    None
}

pub fn read_local(path: &Path) -> Result<Local, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} could not be read: {}", path.display(), e))?;
    if text.trim().is_empty() {
        return Ok(Local::default());
    }
    serde_yaml_ng::from_str(&text).map_err(|e| format!("{} is not valid YAML: {}", path.display(), e))
}

pub fn is_secret(key: &str) -> bool {
    let key = key.to_lowercase();
    SECRET_MARKS.iter().any(|mark| key.contains(mark))
}

pub fn refused(path: &Path, local: &Local) -> Option<Refusal> {
    local.values.keys().find(|key| is_secret(key)).map(|key| Refusal { file: path.to_path_buf(), key: key.clone() })
}

pub fn env_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} could not be read: {}", path.display(), e))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.trim_start_matches("export ").split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().trim_matches('"').trim_matches('\'').to_string()))
        .collect())
}

pub fn resolve(flag: Option<&str>, global: bool, env_files: &[(String, String)], file: &File, local: Option<(PathBuf, Local)>) -> Settings {
    let local = if global { None } else { local };
    let (name, named_by) = match (flag, std::env::var(CONFIG_ENV).ok().filter(|v| !v.is_empty()), local.as_ref().and_then(|(_, l)| l.use_config.clone()), file.default.clone()) {
        (Some(name), _, _, _) => (Some(name.to_string()), Some("flag".to_string())),
        (None, Some(name), _, _) => (Some(name), Some(CONFIG_ENV.to_string())),
        (None, None, Some(name), _) => (Some(name), Some("local".to_string())),
        (None, None, None, Some(name)) => (Some(name), Some("default".to_string())),
        _ => (None, None),
    };
    let named = name.as_ref().and_then(|n| file.configs.get(n)).cloned().unwrap_or_default();
    let mut values = Vec::new();
    for (key, env) in KEYS {
        let found = std::env::var(env)
            .ok()
            .filter(|v| !v.is_empty())
            .map(|v| (v, "environment".to_string()))
            .or_else(|| env_files.iter().find(|(k, _)| k == env).map(|(_, v)| (v.clone(), "env file".to_string())))
            .or_else(|| local.as_ref().and_then(|(_, l)| l.values.get(key).cloned()).map(|v| (v, "local".to_string())))
            .or_else(|| named.get(key).cloned().map(|v| (v, format!("config {}", name.clone().unwrap_or_default()))));
        if let Some((value, layer)) = found {
            values.push(Value { key: key.to_string(), value, layer });
        }
    }
    Settings { name, named_by, values, local: local.map(|(path, _)| path) }
}

impl Settings {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.iter().find(|v| v.key == key).map(|v| v.value.as_str())
    }

    pub fn apply(&self) {
        for (key, env) in KEYS {
            if std::env::var(env).ok().filter(|v| !v.is_empty()).is_some() {
                continue;
            }
            if let Some(value) = self.get(key) {
                let value = if key == "home" { super::plugin::expand_home(value) } else { value.to_string() };
                std::env::set_var(env, value);
            }
        }
    }
}

pub fn seeded() -> File {
    let mut configs = BTreeMap::new();
    configs.insert("app".to_string(), BTreeMap::new());
    configs.insert("mock".to_string(), BTreeMap::from([("home".to_string(), "~/.chatnip-mock".to_string()), ("use_debug".to_string(), "true".to_string())]));
    File { default: Some("app".to_string()), configs }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> File {
        let mut file = seeded();
        file.configs.insert("work".to_string(), BTreeMap::from([("home".to_string(), "/tmp/work".to_string())]));
        file
    }

    #[test]
    fn the_flag_beats_the_local_use_which_beats_the_default() {
        std::env::remove_var(CONFIG_ENV);
        std::env::remove_var("CHATNIP_HOME");
        let local = Local { use_config: Some("work".into()), values: BTreeMap::new() };
        let from_flag = resolve(Some("mock"), false, &[], &file(), Some((PathBuf::from("/repo/.chatnip/config.yaml"), local.clone())));
        assert_eq!((from_flag.name.as_deref(), from_flag.get("home")), (Some("mock"), Some("~/.chatnip-mock")));
        let from_local = resolve(None, false, &[], &file(), Some((PathBuf::from("/repo/.chatnip/config.yaml"), local.clone())));
        assert_eq!((from_local.name.as_deref(), from_local.get("home"), from_local.named_by.as_deref()), (Some("work"), Some("/tmp/work"), Some("local")));
        let global = resolve(None, true, &[], &file(), Some((PathBuf::from("/repo/.chatnip/config.yaml"), local)));
        assert_eq!(global.name.as_deref(), Some("app"));
        assert_eq!(global.get("home"), None);
    }

    #[test]
    fn local_values_override_the_named_config_and_env_files_override_local() {
        std::env::remove_var(CONFIG_ENV);
        std::env::remove_var("CHATNIP_HOME");
        let local = Local { use_config: Some("mock".into()), values: BTreeMap::from([("home".to_string(), "/repo/home".to_string())]) };
        let settings = resolve(None, false, &[], &file(), Some((PathBuf::from("/repo/.chatnip/config.yaml"), local.clone())));
        assert_eq!(settings.values.iter().find(|v| v.key == "home").map(|v| (v.value.as_str(), v.layer.as_str())), Some(("/repo/home", "local")));
        let filed = resolve(None, false, &[("CHATNIP_HOME".into(), "/env/home".into())], &file(), Some((PathBuf::from("/repo/.chatnip/config.yaml"), local)));
        assert_eq!(filed.values.iter().find(|v| v.key == "home").map(|v| v.layer.as_str()), Some("env file"));
    }

    #[test]
    fn a_secret_in_the_local_directory_is_refused() {
        let local = Local { use_config: None, values: BTreeMap::from([("slack_app_token".to_string(), "xapp-1".to_string())]) };
        let refusal = refused(Path::new("/repo/.chatnip/config.yaml"), &local).unwrap();
        assert_eq!(refusal.key, "slack_app_token");
        assert!(refused(Path::new("/repo/.chatnip/config.yaml"), &Local { use_config: Some("mock".into()), values: BTreeMap::new() }).is_none());
    }

    #[test]
    fn env_files_read_keys_and_values() {
        let dir = std::env::temp_dir().join(format!("chatnip-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "# comment\nexport CHATNIP_HOME=\"/x\"\nCHATNIP_USE_DEBUG=true\n").unwrap();
        assert_eq!(env_file(&path).unwrap(), vec![("CHATNIP_HOME".to_string(), "/x".to_string()), ("CHATNIP_USE_DEBUG".to_string(), "true".to_string())]);
    }
}
