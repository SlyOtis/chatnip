pub mod config;
pub mod engine;
pub mod mcp;
pub mod plugin;
pub mod registry;
pub mod style;

use clap::{Arg, ArgAction, ArgMatches, Command};
use registry::{CommandSpec, COMMANDS};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::path::PathBuf;

pub const TOOLS: [&str; 10] = ["help", "version", "commands", "call", "skill", "plugin", "mcp", "completion", "path", "config"];
pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_REFUSED: i32 = 3;
pub const EXIT_AUTH: i32 = 4;
pub const EXIT_INTERRUPTED: i32 = 130;

pub fn wants_cli(args: &[String]) -> bool {
    args.iter().skip(1).any(|arg| !arg.starts_with("-psn_"))
}

fn result_out() -> std::fs::File {
    unsafe {
        let fd = libc::dup(libc::STDOUT_FILENO);
        libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO);
        std::fs::File::from_raw_fd(fd)
    }
}

fn flag(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).action(ArgAction::SetTrue).help(help)
}

fn valued(name: &'static str, value: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).value_name(value).help(help)
}

fn leaf(name: String, spec: &'static CommandSpec) -> Command {
    let mut command = Command::new(name).about(registry::summary(spec)).after_help(format!("Example:\n  {}", registry::example(spec)));
    for param in spec.params {
        let schema = registry::schema(param);
        let boolean = schema.get("type").and_then(Value::as_str) == Some("boolean");
        let mut arg = Arg::new(param.key).long(registry::kebab(param.key)).value_name(param.rust.to_uppercase()).help(format!("{}{}", param.rust, if param.required { ", required" } else { "" }));
        if boolean {
            arg = arg.num_args(0..=1).default_missing_value("true");
        }
        command = command.arg(arg);
    }
    let taken: Vec<String> = spec.params.iter().flat_map(|p| [p.key.to_string(), registry::kebab(p.key)]).collect();
    let shaping = [
        valued("data", "JSON|@FILE", "The whole argument object as JSON, or @file to read it"),
        valued("file", "PATH|-", "Read the argument object from a file, or - for stdin"),
        valued("set", "KEY=VALUE", "Set one argument, dotted keys nest, repeatable").action(ArgAction::Append),
        flag("json", "Print json even on a terminal"),
        flag("raw", "Print the whole reply as compact json"),
        flag("plain", "Print tab separated lines"),
        valued("format", "concise|detailed", "How much of each row to print").value_parser(["concise", "detailed"]),
        valued("limit", "N", "Show at most N rows").value_parser(clap::value_parser!(usize)),
        valued("cursor", "N", "Start at row N, as the last truncated list said").value_parser(clap::value_parser!(usize)),
        valued("fields", "A,B.C", "Only these fields, dotted keys reach inside"),
        valued("sort", "FIELD[:desc]", "Sort the rows by a field"),
        valued("query", "KEY=VALUE", "Keep rows whose field contains the value, repeatable").action(ArgAction::Append),
        valued("jq", "EXPR", "Filter the json through jq"),
        valued("template", "TEXT|@FILE", "Render through a template with json, join, pluck, timeago, color and truncate"),
    ];
    for arg in shaping {
        if !taken.iter().any(|t| t == arg.get_id().as_str()) {
            command = command.arg(arg);
        }
    }
    command
}

const HELP: &str = "{about-with-newline}\nUsage:\n  {usage}\n\n{all-args}{after-help}";

fn templated(command: Command) -> Command {
    command.help_template(HELP).mut_subcommands(templated)
}

pub fn tree() -> Command {
    let mut root = Command::new(plugin::NAME)
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .arg(valued("config", "NAME", "Use this named config from ~/.chatnip/config.yaml").global(true))
        .arg(flag("global", "Ignore the local .chatnip/config.yaml").global(true))
        .arg(valued("env", "FILE", "Load every key of a dotenv file, repeatable").global(true).action(ArgAction::Append))
        .subcommand(Command::new("help").about("Print the manual, or the help of one command").arg(Arg::new("topic").num_args(0..)))
        .subcommand(Command::new("version").about("Print the version and the running engine"))
        .subcommand(Command::new("commands").about("List every resource command with its flags").arg(flag("json", "Print json")))
        .subcommand(
            Command::new("call")
                .about("Run any engine command by its rust name")
                .arg(Arg::new("name").required(true))
                .arg(valued("data", "JSON|@FILE", "The whole argument object as JSON, or @file to read it"))
                .arg(flag("raw", "Print the whole reply as compact json")),
        )
        .subcommand(
            Command::new("skill")
                .about("The claude skill that teaches an agent to drive chatnip")
                .subcommand(Command::new("get").about("Print the skill"))
                .subcommand(Command::new("install").about("Install the skill under ~/.claude/skills, or ./.claude with --project").arg(flag("project", "Install for this project"))),
        )
        .subcommand(
            Command::new("plugin")
                .about("The claude plugin with the skill and the chatnip mcp server")
                .after_help("chatnip plugin install [--config NAME] [--project]\n  --config NAME  the named config the mcp server runs with, such as mock; the app data when left out\nchatnip plugin status prints the config, home and mode the installed mcp server uses")
                .subcommand(Command::new("install").about("Write ~/.chatnip/plugin and install chatnip@chatnip through the claude cli").arg(flag("project", "Install for this project instead of the user")))
                .subcommand(Command::new("status").about("Show what is installed, with the data home and mode its mcp server uses")),
        )
        .subcommand(Command::new("mcp").about("Serve every engine command as a tool over the model context protocol on stdio"))
        .subcommand(
            Command::new("completion")
                .about("Print shell completion, or install it with --auto")
                .arg(Arg::new("shell").value_parser(clap::value_parser!(clap_complete::Shell)))
                .arg(flag("auto", "Write the completion for the current shell into the XDG data folder")),
        )
        .subcommand(
            Command::new("path")
                .about("Put chatnip on the PATH")
                .subcommand(Command::new("install").about("Link this binary into ~/.local/bin, or --dir").arg(valued("dir", "DIR", "Link into this folder"))),
        )
        .subcommand(
            Command::new("config")
                .about("Named configs in ~/.chatnip/config.yaml and the local .chatnip/config.yaml")
                .subcommand(Command::new("get").about("Show each value and the layer it comes from"))
                .subcommand(Command::new("set").about("Set a key in a named config").arg(Arg::new("key").required(true)).arg(Arg::new("value").required(true)))
                .subcommand(Command::new("use").about("Make a named config the default").arg(Arg::new("name").required(true)))
                .subcommand(Command::new("local").about("Pin this folder to a named config, or off to remove the pin").arg(Arg::new("name").required(true))),
        );
    let mut resources: BTreeMap<String, Vec<(String, &'static CommandSpec)>> = BTreeMap::new();
    for spec in COMMANDS {
        let route = registry::route(spec.name);
        if let Some(verb) = route.verb {
            resources.entry(route.resource).or_default().push((verb, spec));
        }
    }
    for (resource, verbs) in resources {
        let about = verbs.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>().join(", ");
        let mut command = Command::new(resource).about(about).subcommand_required(true);
        for (verb, spec) in verbs {
            command = command.subcommand(leaf(verb, spec));
        }
        root = root.subcommand(command);
    }
    templated(root)
}

fn print_help() {
    print!("{}", plugin::SKILL);
}

fn print_commands(json: bool, out: &mut dyn Write) {
    if json {
        let list: Vec<Value> = COMMANDS
            .iter()
            .map(|spec| {
                let route = registry::route(spec.name);
                serde_json::json!({ "name": spec.name, "tool": registry::tool_name(spec), "resource": route.resource, "verb": route.verb, "summary": registry::summary(spec), "example": registry::example(spec), "input": registry::input_schema(spec) })
            })
            .collect();
        let _ = style::json(out, &Value::Array(list), false);
        return;
    }
    let mut rows: Vec<(String, String, String)> = COMMANDS
        .iter()
        .map(|spec| {
            let route = registry::route(spec.name);
            let words = format!("{} {}", route.resource, route.verb.unwrap_or_default());
            let flags = spec.params.iter().map(|p| if p.required { format!("--{}", registry::kebab(p.key)) } else { format!("[--{}]", registry::kebab(p.key)) }).collect::<Vec<_>>().join(" ");
            (words, flags, registry::summary(spec))
        })
        .collect();
    rows.sort();
    for (words, flags, summary) in rows {
        let _ = writeln!(out, "{}  {}", style::line(&words, &summary), style::dim(&flags));
    }
}

fn read_data(raw: Option<&String>) -> Result<Option<Value>, String> {
    let Some(raw) = raw else { return Ok(None) };
    let text = match raw.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{} could not be read: {}", path, e))?,
        None => raw.clone(),
    };
    serde_json::from_str(&text).map(Some).map_err(|e| format!("--data is not JSON: {}", e))
}

fn read_file(raw: Option<&String>) -> Result<Option<Value>, String> {
    let Some(path) = raw else { return Ok(None) };
    let text = if path == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).map_err(|e| format!("stdin could not be read: {}", e))?;
        text
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{} could not be read: {}", path, e))?
    };
    serde_json::from_str(&text).map(Some).map_err(|e| format!("--file is not JSON: {}", e))
}

fn error_text(error: &Value) -> String {
    match error {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

pub fn classify(error: &str) -> (i32, Option<&'static str>) {
    let lower = error.to_lowercase();
    if lower.contains("is locked") || lower.contains("stayed locked") {
        return (EXIT_AUTH, Some("unlock with chatnip lock delete, or open Chatnip and unlock it"));
    }
    if ["no auth tokens", "refused the token", "not connected", "token expired", "expired token", "invalid_auth", "token_revoked", "401", "unauthorized", "reconnect", "sign in again"].iter().any(|m| lower.contains(m)) {
        return (EXIT_AUTH, Some("reconnect the account under Settings, Connections; chatnip providers get shows which"));
    }
    if ["403", "forbidden", "not allowed", "denied", "refused", "missing_scope", "not_in_channel", "cannot "].iter().any(|m| lower.contains(m)) {
        return (EXIT_REFUSED, Some("the provider refused it; chatnip providers get shows each account's capabilities"));
    }
    if ["missing --", "is not json", "must be a json", "needs key=value", "not an rfc 3339", "unknown field", "invalid type"].iter().any(|m| lower.contains(m)) {
        return (EXIT_USAGE, Some("chatnip help <resource> <verb> shows the flags and an example"));
    }
    (EXIT_FAILURE, None)
}

fn fail(error: &str) -> i32 {
    let (code, hint) = classify(error);
    fail_as(code, error, hint)
}

fn fail_as(code: i32, error: &str, hint: Option<&str>) -> i32 {
    style::fail(error);
    if let Some(hint) = hint {
        eprintln!("  {}", style::dim(hint));
    }
    code
}

fn shape_of(matches: &ArgMatches, spec: &CommandSpec) -> Result<style::Shape, String> {
    let own = |name: &str| !spec.params.iter().any(|p| p.key == name || registry::kebab(p.key) == name);
    let pairs = |name: &str| -> Result<Vec<(String, String)>, String> {
        matches
            .get_many::<String>(name)
            .map(|values| values.map(|v| v.split_once('=').map(|(k, v)| (k.trim().to_string(), v.to_string())).ok_or_else(|| format!("--{} {} needs key=value", name, v))).collect::<Result<Vec<_>, _>>())
            .unwrap_or_else(|| Ok(Vec::new()))
    };
    Ok(style::Shape {
        json: matches.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false),
        raw: matches.try_get_one::<bool>("raw").ok().flatten().copied().unwrap_or(false),
        plain: matches.try_get_one::<bool>("plain").ok().flatten().copied().unwrap_or(false),
        detailed: matches.try_get_one::<String>("format").ok().flatten().map(|f| f == "detailed").unwrap_or(false),
        limit: if !own("limit") { None } else { matches.try_get_one::<usize>("limit").ok().flatten().copied() },
        cursor: matches.try_get_one::<usize>("cursor").ok().flatten().copied(),
        fields: matches.try_get_one::<String>("fields").ok().flatten().map(|f| f.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        sort: matches.try_get_one::<String>("sort").ok().flatten().map(|s| match s.rsplit_once(':') {
            Some((field, "desc")) => (field.to_string(), true),
            Some((field, _)) => (field.to_string(), false),
            None => (s.clone(), false),
        }),
        query: if own("query") && matches.try_contains_id("query").unwrap_or(false) { pairs("query")? } else { Vec::new() },
        jq: matches.try_get_one::<String>("jq").ok().flatten().cloned(),
        template: matches.try_get_one::<String>("template").ok().flatten().cloned(),
    })
}

fn execute(spec: &'static CommandSpec, matches: &ArgMatches) -> i32 {
    let data = match (read_data(matches.try_get_one::<String>("data").ok().flatten()), read_file(matches.try_get_one::<String>("file").ok().flatten())) {
        (Ok(data), Ok(None)) => data,
        (Ok(None), Ok(file)) => file,
        (Ok(Some(_)), Ok(Some(_))) => return fail_as(EXIT_USAGE, "pass --data or --file, not both", None),
        (Err(e), _) | (_, Err(e)) => return fail(&e),
    };
    let flags: Vec<(&registry::ParamSpec, String)> = spec.params.iter().filter_map(|p| matches.try_get_one::<String>(p.key).ok().flatten().map(|v| (p, v.clone()))).collect();
    let sets: Vec<String> = matches.try_get_many::<String>("set").ok().flatten().map(|v| v.cloned().collect()).unwrap_or_default();
    let args = match registry::arguments(spec, data, &flags, &sets) {
        Ok(args) => args,
        Err(e) => return fail(&e),
    };
    let shape = match shape_of(matches, spec) {
        Ok(shape) => shape,
        Err(e) => return fail(&e),
    };
    let terminal = style::stdout_is_terminal();
    let mut out = result_out();
    match run_command(spec, args) {
        Ok(value) => match style::emit(&mut out, value, &shape, terminal) {
            Ok(()) => EXIT_OK,
            Err(e) => fail(&e),
        },
        Err(error) => fail(&error_text(&error)),
    }
}

pub fn step(index: usize, total: usize, message: &str) {
    if std::env::var_os("CHATNIP_MCP").is_some() {
        eprintln!("step {}", serde_json::json!({ "progress": index, "total": total, "message": message }));
    } else {
        style::step(message);
    }
}

pub fn run_command(spec: &'static CommandSpec, args: Value) -> Result<Value, Value> {
    if spec.name != "post_sync" {
        return engine::dispatch(Box::new(move |call| call(spec.name, args)));
    }
    engine::dispatch(Box::new(|call| {
        let providers = call("get_providers", serde_json::json!({}))?;
        let enabled: Vec<(String, String)> = providers["data"].as_array().cloned().unwrap_or_default().iter().filter(|p| p["is_enabled"].as_bool() != Some(false)).map(|p| (p["id"].as_str().unwrap_or_default().to_string(), p["name"].as_str().unwrap_or_default().to_string())).collect();
        let total = enabled.len();
        let mut failed = Vec::new();
        for (index, (id, name)) in enabled.iter().enumerate() {
            step(index, total, &format!("syncing {}", name));
            if let Err(error) = call("post_provider_sync", serde_json::json!({ "providerId": id })) {
                failed.push(serde_json::json!({ "provider_id": id, "name": name, "error": error_text(&error) }));
            }
        }
        step(total, total, "reading the inbox");
        let mut conversations = call("get_conversations", serde_json::json!({}))?;
        if let Value::Object(map) = &mut conversations {
            map.insert("failed".to_string(), Value::Array(failed));
        }
        Ok(conversations)
    }))
}

fn call(matches: &ArgMatches) -> i32 {
    let name = matches.get_one::<String>("name").cloned().unwrap_or_default();
    let Some(spec) = COMMANDS.iter().find(|spec| spec.name == name) else {
        style::fail(&format!("no engine command is called {}; chatnip commands lists them", name));
        return EXIT_USAGE;
    };
    let data = match read_data(matches.get_one::<String>("data")) {
        Ok(data) => data,
        Err(e) => return fail(&e),
    };
    let args = match registry::arguments(spec, data, &[], &[]) {
        Ok(args) => args,
        Err(e) => return fail(&e),
    };
    let mut out = result_out();
    match run_command(spec, args) {
        Ok(value) => {
            let _ = style::json(&mut out, &value, matches.get_flag("raw"));
            EXIT_OK
        }
        Err(error) => fail(&error_text(&error)),
    }
}

fn current_settings(matches: &ArgMatches) -> Result<config::Settings, String> {
    let file = Some(config::read_file(&config::home_file())?).filter(|f| !f.configs.is_empty()).unwrap_or_else(config::seeded);
    let local = match std::env::current_dir().ok().and_then(|dir| config::find_local(&dir)) {
        Some(path) => Some((path.clone(), config::read_local(&path)?)),
        None => None,
    };
    let mut env_files = Vec::new();
    for path in matches.get_many::<String>("env").map(|v| v.cloned().collect::<Vec<_>>()).unwrap_or_default() {
        env_files.extend(config::env_file(std::path::Path::new(&path))?);
    }
    Ok(config::resolve(matches.get_one::<String>("config").map(String::as_str), matches.get_flag("global"), &env_files, &file, local))
}

fn plugin_options(matches: &ArgMatches) -> Result<plugin::PluginOptions, String> {
    let Some(name) = matches.get_one::<String>("config") else { return Ok(plugin::PluginOptions::default()) };
    let file = Some(config::read_file(&config::home_file())?).filter(|f| !f.configs.is_empty()).unwrap_or_else(config::seeded);
    let chosen = file.configs.get(name).ok_or_else(|| format!("there is no config called {}; chatnip config get lists them", name))?;
    Ok(plugin::PluginOptions { home: chosen.get("home").cloned(), mock: chosen.get("use_debug").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false) })
}

fn config_tool(matches: &ArgMatches, root: &ArgMatches) -> i32 {
    let path = config::home_file();
    let mut file = match config::read_file(&path) {
        Ok(file) if file.configs.is_empty() => config::seeded(),
        Ok(file) => file,
        Err(e) => return fail(&e),
    };
    match matches.subcommand() {
        Some(("set", sub)) => {
            let key = sub.get_one::<String>("key").cloned().unwrap_or_default();
            if !config::KEYS.iter().any(|(k, _)| *k == key) {
                return fail_as(EXIT_USAGE, &format!("{} is not a config key; the keys are {}", key, config::KEYS.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")), None);
            }
            let name = root.get_one::<String>("config").cloned().or(file.default.clone()).unwrap_or_else(|| "app".to_string());
            file.configs.entry(name.clone()).or_default().insert(key.clone(), sub.get_one::<String>("value").cloned().unwrap_or_default());
            match config::write_file(&path, &file) {
                Ok(()) => {
                    style::ok(&format!("set {} in config {}", key, name));
                    EXIT_OK
                }
                Err(e) => fail(&e),
            }
        }
        Some(("use", sub)) => {
            let name = sub.get_one::<String>("name").cloned().unwrap_or_default();
            if !file.configs.contains_key(&name) {
                return fail_as(EXIT_USAGE, &format!("there is no config called {}", name), Some("chatnip config get lists them"));
            }
            file.default = Some(name.clone());
            match config::write_file(&path, &file) {
                Ok(()) => {
                    style::ok(&format!("{} is the default config", name));
                    EXIT_OK
                }
                Err(e) => fail(&e),
            }
        }
        Some(("local", sub)) => {
            let name = sub.get_one::<String>("name").cloned().unwrap_or_default();
            let dir = std::env::current_dir().unwrap_or_default().join(config::LOCAL_DIR);
            let target = dir.join(config::FILE);
            if name == "off" {
                return match std::fs::remove_file(&target) {
                    Ok(()) | Err(_) => {
                        style::ok(&format!("{} no longer pins a config", dir.display()));
                        EXIT_OK
                    }
                };
            }
            if !file.configs.contains_key(&name) {
                return fail_as(EXIT_USAGE, &format!("there is no config called {}", name), Some("chatnip config get lists them"));
            }
            let _ = std::fs::create_dir_all(&dir);
            match std::fs::write(&target, format!("use: {}\n", name)) {
                Ok(()) => {
                    style::ok(&format!("{} uses config {}", dir.display(), name));
                    EXIT_OK
                }
                Err(e) => fail(&format!("{} could not be written: {}", target.display(), e)),
            }
        }
        _ => {
            let settings = match current_settings(root) {
                Ok(settings) => settings,
                Err(e) => return fail(&e),
            };
            let terminal = style::stdout_is_terminal();
            let value = serde_json::json!({
                "config": settings.name,
                "chosen_by": settings.named_by,
                "file": path.display().to_string(),
                "local": settings.local.as_ref().map(|p| p.display().to_string()),
                "configs": file.configs.keys().collect::<Vec<_>>(),
                "values": settings.values.iter().map(|v| serde_json::json!({ "key": v.key, "value": if config::is_secret(&v.key) { "••••".to_string() } else { v.value.clone() }, "layer": v.layer })).collect::<Vec<_>>(),
            });
            let mut out = result_out();
            match style::emit(&mut out, value, &style::Shape::default(), terminal) {
                Ok(()) => EXIT_OK,
                Err(e) => fail(&e),
            }
        }
    }
}

fn completion(matches: &ArgMatches) -> i32 {
    let shell = matches.get_one::<clap_complete::Shell>("shell").copied().or_else(clap_complete::Shell::from_env);
    let Some(shell) = shell else {
        return fail_as(EXIT_USAGE, "name the shell, such as chatnip completion zsh", None);
    };
    if !matches.get_flag("auto") {
        clap_complete::generate(shell, &mut tree(), plugin::NAME, &mut std::io::stdout());
        return EXIT_OK;
    }
    let dir = dirs::data_dir().unwrap_or_else(|| plugin::home().join(".local/share")).join(plugin::NAME).join("completion");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(&format!("{} could not be created: {}", dir.display(), e));
    }
    let file = dir.join(match shell {
        clap_complete::Shell::Zsh => format!("_{}", plugin::NAME),
        clap_complete::Shell::Fish => format!("{}.fish", plugin::NAME),
        _ => format!("{}.{}", plugin::NAME, shell),
    });
    let mut buffer = Vec::new();
    clap_complete::generate(shell, &mut tree(), plugin::NAME, &mut buffer);
    match std::fs::write(&file, buffer) {
        Ok(()) => {
            style::ok(&format!("wrote {}", file.display()));
            match shell {
                clap_complete::Shell::Zsh => eprintln!("  {}", style::dim(&format!("add fpath=({} $fpath) before compinit in ~/.zshrc", dir.display()))),
                _ => eprintln!("  {}", style::dim(&format!("source {} from your shell profile", file.display()))),
            }
            EXIT_OK
        }
        Err(e) => fail(&format!("{} could not be written: {}", file.display(), e)),
    }
}

fn tool(name: &str, matches: &ArgMatches, root: &ArgMatches) -> i32 {
    match name {
        "help" => {
            let topic: Vec<String> = matches.get_many::<String>("topic").map(|v| v.cloned().collect()).unwrap_or_default();
            if topic.is_empty() {
                print_help();
                return EXIT_OK;
            }
            let mut command = tree();
            command.build();
            let mut current = &mut command;
            for word in &topic {
                match current.find_subcommand_mut(word) {
                    Some(next) => current = next,
                    None => return fail_as(EXIT_USAGE, &format!("no command is called {}", topic.join(" ")), Some("chatnip commands lists them")),
                }
            }
            let _ = current.print_long_help();
            EXIT_OK
        }
        "version" => {
            println!("chatnip {}", env!("CARGO_PKG_VERSION"));
            println!("engine {}", if engine::running() { "the open app" } else { "headless when called" });
            EXIT_OK
        }
        "commands" => {
            print_commands(matches.get_flag("json"), &mut std::io::stdout());
            EXIT_OK
        }
        "call" => call(matches),
        "skill" => match matches.subcommand() {
            Some(("install", sub)) => {
                let base = if sub.get_flag("project") { PathBuf::from(".claude") } else { plugin::home().join(".claude") };
                match plugin::put_skill(&base) {
                    Ok(dest) => {
                        style::ok(&format!("installed the skill into {}", dest.display()));
                        EXIT_OK
                    }
                    Err(e) => fail(&e),
                }
            }
            _ => {
                print!("{}", plugin::SKILL);
                EXIT_OK
            }
        },
        "plugin" => match matches.subcommand() {
            Some(("install", sub)) => {
                let options = match plugin_options(root) {
                    Ok(options) => options,
                    Err(e) => return fail(&e),
                };
                match plugin::install_plugin(sub.get_flag("project"), &options) {
                    Ok(steps) => {
                        steps.iter().for_each(|s| style::ok(s));
                        EXIT_OK
                    }
                    Err(e) => fail(&e),
                }
            }
            _ => {
                let setup = plugin::setup();
                style::section("Claude plugin");
                style::field("claude", setup.claude.as_deref().unwrap_or("not found"));
                style::field("installed", if setup.plugin_installed { "yes" } else { "no" });
                style::field("written", setup.plugin_version.as_deref().unwrap_or("no"));
                style::field("this binary", &setup.plugin_current);
                style::field("folder", &setup.plugin_dir);
                if let Some(options) = &setup.plugin_options {
                    style::field("mcp home", options.home.as_deref().unwrap_or("app data"));
                    style::field("mock mode", if options.mock { "forced on" } else { "from settings" });
                }
                style::section("Command line");
                style::field("binary", &setup.executable);
                style::field("link", &format!("{}{}", setup.link, if setup.linked { "" } else { " (not linked)" }));
                style::field("on PATH", if setup.on_path { "yes" } else { "no" });
                if setup.plugin_version.as_deref() != Some(setup.plugin_current.as_str()) {
                    style::warn("run chatnip plugin install to bring the plugin up to date");
                }
                EXIT_OK
            }
        },
        "mcp" => mcp::serve(result_out()),
        "completion" => completion(matches),
        "config" => config_tool(matches, root),
        "path" => {
            let dir = matches.subcommand().and_then(|(_, sub)| sub.get_one::<String>("dir")).map(PathBuf::from);
            match plugin::link(dir) {
                Ok(target) => {
                    style::ok(&format!("linked {}", target.display()));
                    if let Some(parent) = target.parent() {
                        if !plugin::on_path(parent) {
                            style::warn(&format!("{} is not on your PATH; add it in your shell profile", parent.display()));
                        }
                    }
                    EXIT_OK
                }
                Err(e) => fail(&e),
            }
        }
        _ => EXIT_USAGE,
    }
}

pub fn configure(args: &[String]) -> Result<(), (i32, String)> {
    let usage = |e: String| (EXIT_USAGE, e);
    let matches = tree().ignore_errors(true).try_get_matches_from(args).ok();
    let file = Some(config::read_file(&config::home_file()).map_err(usage)?).filter(|f| !f.configs.is_empty()).unwrap_or_else(config::seeded);
    let global = matches.as_ref().map(|m| m.get_flag("global")).unwrap_or(false);
    let local = match std::env::current_dir().ok().and_then(|dir| config::find_local(&dir)).filter(|_| !global) {
        Some(path) => {
            let local = config::read_local(&path).map_err(usage)?;
            if let Some(refusal) = config::refused(&path, &local) {
                return Err((EXIT_AUTH, format!("{} holds the secret {}; secrets live only in ~/.chatnip/config.yaml", refusal.file.display(), refusal.key)));
            }
            Some((path, local))
        }
        None => None,
    };
    let mut env_files = Vec::new();
    for path in matches.as_ref().and_then(|m| m.get_many::<String>("env")).map(|v| v.cloned().collect::<Vec<_>>()).unwrap_or_default() {
        let pairs = config::env_file(std::path::Path::new(&path)).map_err(usage)?;
        for (key, value) in &pairs {
            if std::env::var(key).ok().filter(|v| !v.is_empty()).is_none() {
                std::env::set_var(key, value);
            }
        }
        env_files.extend(pairs);
    }
    let chosen = matches.as_ref().and_then(|m| m.get_one::<String>("config").cloned());
    config::resolve(chosen.as_deref(), global, &env_files, &file, local).apply();
    Ok(())
}

pub fn run(args: Vec<String>) -> i32 {
    let args: Vec<String> = args.into_iter().filter(|a| !a.starts_with("-psn_")).collect();
    if args.len() < 2 || matches!(args[1].as_str(), "-h" | "--help") {
        print_help();
        return EXIT_OK;
    }
    if args[1] == "--version" || args[1] == "-V" {
        println!("chatnip {}", env!("CARGO_PKG_VERSION"));
        return EXIT_OK;
    }
    let matches = match tree().try_get_matches_from(&args) {
        Ok(matches) => matches,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { EXIT_USAGE } else { EXIT_OK };
        }
    };
    let Some((name, sub)) = matches.subcommand() else {
        print_help();
        return EXIT_OK;
    };
    if TOOLS.contains(&name) {
        return tool(name, sub, &matches);
    }
    match sub.subcommand() {
        Some((verb, leaf)) => match registry::find(name, Some(verb)) {
            Some(spec) => execute(spec, leaf),
            None => EXIT_USAGE,
        },
        None => EXIT_USAGE,
    }
}

pub fn interrupted() -> i32 {
    EXIT_INTERRUPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_opens_without_arguments_and_the_cli_runs_with_them() {
        assert!(!wants_cli(&["chatnip".into()]));
        assert!(!wants_cli(&["chatnip".into(), "-psn_0_12345".into()]));
        assert!(wants_cli(&["chatnip".into(), "conversations".into(), "get".into()]));
        assert!(wants_cli(&["chatnip".into(), "--help".into()]));
    }

    #[test]
    fn the_tree_parses_resource_formula_verbs_and_shape_flags() {
        tree().debug_assert();
        let matches = tree().try_get_matches_from(["chatnip", "action-code", "get", "--action-id", "m1", "--raw", "--limit", "5", "--set", "a.b=1"]).unwrap();
        let (resource, sub) = matches.subcommand().unwrap();
        assert_eq!(resource, "action-code");
        let (verb, leaf) = sub.subcommand().unwrap();
        assert_eq!(verb, "get");
        assert_eq!(leaf.get_one::<String>("actionId").unwrap(), "m1");
        assert!(leaf.get_flag("raw"));
        assert_eq!(leaf.get_one::<usize>("limit"), Some(&5));
        assert!(tree().try_get_matches_from(["chatnip", "message", "create", "--input", "{}"]).is_ok());
        assert!(tree().try_get_matches_from(["chatnip", "message", "post"]).is_err());
        assert!(tree().try_get_matches_from(["chatnip", "lock", "delete"]).is_ok());
        assert!(tree().try_get_matches_from(["chatnip", "plugin", "install", "--project", "--config", "mock"]).is_ok());
        assert!(tree().try_get_matches_from(["chatnip", "skill", "install"]).is_ok());
        assert!(tree().try_get_matches_from(["chatnip", "skill", "put"]).is_err());
        assert!(tree().try_get_matches_from(["chatnip", "config", "get"]).is_ok());
        assert!(tree().try_get_matches_from(["chatnip", "completion", "--auto"]).is_ok());
    }

    #[test]
    fn errors_map_to_the_family_exit_codes() {
        assert_eq!(classify("Chatnip is locked").0, EXIT_AUTH);
        assert_eq!(classify("No auth tokens available").0, EXIT_AUTH);
        assert_eq!(classify("Slack: invalid_auth").0, EXIT_AUTH);
        assert_eq!(classify("Graph refused the call: 403 Forbidden").0, EXIT_REFUSED);
        assert_eq!(classify("missing --conversation-id").0, EXIT_USAGE);
        assert_eq!(classify("--data is not JSON: expected value").0, EXIT_USAGE);
        assert_eq!(classify("Failed to save message: disk").0, EXIT_FAILURE);
        assert!(classify("missing --x").1.unwrap().contains("chatnip help"));
        assert_eq!(interrupted(), 130);
    }

    #[test]
    fn json_goes_to_a_pipe_and_a_table_to_a_terminal() {
        let shape = style::Shape::default();
        assert_eq!(style::mode(&shape, false), style::Mode::Json);
        assert_eq!(style::mode(&shape, true), style::Mode::Table);
        assert_eq!(style::mode(&style::Shape { json: true, ..Default::default() }, true), style::Mode::Json);
        assert_eq!(style::mode(&style::Shape { raw: true, ..Default::default() }, true), style::Mode::Raw);
        assert_eq!(style::mode(&style::Shape { plain: true, ..Default::default() }, false), style::Mode::Plain);
        let rows = serde_json::json!([{ "id": "b", "n": 2, "deep": { "x": 1 } }, { "id": "a", "n": 1 }, { "id": "c", "n": 3 }]);
        let shaped = style::shape(rows.clone(), &style::Shape { limit: Some(2), sort: Some(("n".into(), false)), ..Default::default() });
        assert_eq!(shaped.value, serde_json::json!([{ "id": "a", "n": 1 }, { "id": "b", "n": 2 }]));
        assert_eq!(shaped.truncated, Some((2, 3)));
        let fields = style::shape(rows, &style::Shape { fields: vec!["deep.x".into()], query: vec![("id".into(), "B".into())], ..Default::default() });
        assert_eq!(fields.value, serde_json::json!([{ "deep.x": 1 }]));
    }

    #[test]
    fn data_reads_inline_json_and_files() {
        assert_eq!(read_data(Some(&"{\"a\":1}".to_string())).unwrap(), Some(serde_json::json!({ "a": 1 })));
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), "{\"b\":2}").unwrap();
        assert_eq!(read_data(Some(&format!("@{}", file.path().display()))).unwrap(), Some(serde_json::json!({ "b": 2 })));
        assert_eq!(read_file(Some(&file.path().display().to_string())).unwrap(), Some(serde_json::json!({ "b": 2 })));
        assert!(read_data(Some(&"nope".to_string())).is_err());
    }
}
