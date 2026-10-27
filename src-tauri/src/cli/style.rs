use std::io::Write;
use std::sync::OnceLock;

fn color() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        if std::env::var("CLICOLOR_FORCE").map(|v| v != "0").unwrap_or(false) {
            return true;
        }
        if std::env::var_os("NO_COLOR").is_some() || std::env::var("CLICOLOR").map(|v| v == "0").unwrap_or(false) || std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
            return false;
        }
        unsafe { libc::isatty(libc::STDERR_FILENO) == 1 }
    })
}

fn paint(code: &str, text: &str) -> String {
    if color() {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}

pub fn bold(text: &str) -> String {
    paint("1", text)
}

pub fn dim(text: &str) -> String {
    paint("2", text)
}

pub fn cyan(text: &str) -> String {
    paint("36", text)
}

pub fn green(text: &str) -> String {
    paint("32", text)
}

pub fn yellow(text: &str) -> String {
    paint("33", text)
}

pub fn red(text: &str) -> String {
    paint("31", text)
}

pub fn section(title: &str) {
    eprintln!();
    eprintln!("{} {}", cyan("▌"), bold(title));
}

pub fn field(label: &str, value: &str) {
    eprintln!("  {}  {}", dim(&format!("{:<15}", label)), value);
}

pub fn ok(text: &str) {
    eprintln!("  {} {}", green("✓"), text);
}

pub fn step(text: &str) {
    eprintln!("  {} {}", cyan("→"), text);
}

pub fn warn(text: &str) {
    eprintln!("  {} {}", yellow("!"), text);
}

pub fn fail(text: &str) {
    eprintln!("  {} {}", red("✗"), text);
}

pub fn line(name: &str, description: &str) -> String {
    format!("  {}  {}", cyan(&format!("{:<34}", name)), dim(description))
}

pub fn banner() -> String {
    let art = [
        "      _           _             _",
        "  ___| |__   __ _| |_ _ __  (_)_ __",
        " / __| '_ \\ / _` | __| '_ \\ | | '_ \\",
        "| (__| | | | (_| | |_| | | || | |_) |",
        " \\___|_| |_|\\__,_|\\__|_| |_||_| .__/",
        "                              |_|",
    ];
    let mut out = String::new();
    for row in art {
        out.push_str(&cyan(row));
        out.push('\n');
    }
    out.push_str(&format!("  {}  {}", dim("Every inbox, one engine.  Chatnip © 2026"), dim(env!("CARGO_PKG_VERSION"))));
    out
}

pub fn json(out: &mut dyn Write, value: &serde_json::Value, raw: bool) -> std::io::Result<()> {
    let text = if raw { serde_json::to_string(value) } else { serde_json::to_string_pretty(value) }.unwrap_or_default();
    writeln!(out, "{}", text)?;
    out.flush()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Table,
    Json,
    Raw,
    Plain,
}

#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub json: bool,
    pub raw: bool,
    pub plain: bool,
    pub detailed: bool,
    pub limit: Option<usize>,
    pub cursor: Option<usize>,
    pub fields: Vec<String>,
    pub sort: Option<(String, bool)>,
    pub query: Vec<(String, String)>,
    pub jq: Option<String>,
    pub template: Option<String>,
}

pub fn stdout_is_terminal() -> bool {
    std::env::var_os("CHATNIP_MCP").is_none() && unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
}

pub fn mode(shape: &Shape, terminal: bool) -> Mode {
    match (shape.raw, shape.json, shape.plain, terminal) {
        (true, _, _, _) => Mode::Raw,
        (_, true, _, _) => Mode::Json,
        (_, _, true, _) => Mode::Plain,
        (_, _, _, true) => Mode::Table,
        _ => Mode::Json,
    }
}

fn pick(value: &serde_json::Value, path: &str) -> serde_json::Value {
    path.split('.').fold(value.clone(), |current, key| current.get(key).cloned().unwrap_or(serde_json::Value::Null))
}

fn listed(value: &serde_json::Value) -> Option<(Vec<serde_json::Value>, Option<&'static str>)> {
    match value {
        serde_json::Value::Array(items) => Some((items.clone(), None)),
        serde_json::Value::Object(map) => ["data", "items", "messages", "conversations", "results"].iter().find_map(|key| map.get(*key).and_then(|v| v.as_array()).map(|items| (items.clone(), Some(*key)))),
        _ => None,
    }
}

fn scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn concise(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(map.iter().filter(|(_, v)| !v.is_object() && !v.is_array()).map(|(k, v)| (k.clone(), v.clone())).collect()),
        other => other.clone(),
    }
}

pub struct Shaped {
    pub value: serde_json::Value,
    pub truncated: Option<(usize, usize)>,
}

pub fn shape(value: serde_json::Value, shape: &Shape) -> Shaped {
    let Some((mut items, key)) = listed(&value) else {
        let picked = match (&shape.fields[..], shape.detailed) {
            ([], true) => value,
            ([], false) => value,
            (fields, _) => serde_json::Value::Object(fields.iter().map(|f| (f.clone(), pick(&value, f))).collect()),
        };
        return Shaped { value: picked, truncated: None };
    };
    for (key, wanted) in &shape.query {
        let wanted = wanted.to_lowercase();
        items.retain(|item| scalar(&pick(item, key)).to_lowercase().contains(&wanted));
    }
    if let Some((field, descending)) = &shape.sort {
        items.sort_by(|a, b| {
            let (x, y) = (pick(a, field), pick(b, field));
            let order = match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => scalar(&x).cmp(&scalar(&y)),
            };
            if *descending { order.reverse() } else { order }
        });
    }
    let total = items.len();
    let start = shape.cursor.unwrap_or(0).min(total);
    let end = shape.limit.map(|l| (start + l).min(total)).unwrap_or(total);
    let mut page: Vec<serde_json::Value> = items[start..end].to_vec();
    if !shape.fields.is_empty() {
        page = page.iter().map(|item| serde_json::Value::Object(shape.fields.iter().map(|f| (f.clone(), pick(item, f))).collect())).collect();
    } else if !shape.detailed && shape.limit.is_some() {
        page = page.iter().map(concise).collect();
    }
    let truncated = (end < total).then_some((end, total));
    let value = match (value, key) {
        (serde_json::Value::Object(mut map), Some(key)) => {
            map.insert(key.to_string(), serde_json::Value::Array(page));
            serde_json::Value::Object(map)
        }
        _ => serde_json::Value::Array(page),
    };
    Shaped { value, truncated }
}

pub fn table(out: &mut dyn Write, value: &serde_json::Value) -> std::io::Result<()> {
    let rows: Vec<serde_json::Value> = match listed(value) {
        Some((items, _)) => items,
        None => {
            if let serde_json::Value::Object(map) = value {
                for (key, field) in map {
                    let shown = if field.is_object() || field.is_array() { dim(&field.to_string()) } else { scalar(field) };
                    writeln!(out, "{}  {}", cyan(&format!("{:<22}", key)), shown)?;
                }
                return out.flush();
            }
            writeln!(out, "{}", scalar(value))?;
            return out.flush();
        }
    };
    if rows.is_empty() {
        writeln!(out, "{}", dim("(none)"))?;
        return out.flush();
    }
    let mut columns: Vec<String> = Vec::new();
    for row in rows.iter().take(50) {
        if let serde_json::Value::Object(map) = row {
            for (key, field) in map {
                if !field.is_object() && !field.is_array() && !columns.contains(key) && columns.len() < 6 {
                    columns.push(key.clone());
                }
            }
        }
    }
    if columns.is_empty() {
        for row in &rows {
            writeln!(out, "{}", scalar(row))?;
        }
        return out.flush();
    }
    let cell = |row: &serde_json::Value, column: &str| -> String {
        let text = scalar(&row[column]).replace('\n', " ");
        if text.chars().count() > 48 { format!("{}…", text.chars().take(47).collect::<String>()) } else { text }
    };
    let widths: Vec<usize> = columns.iter().map(|c| rows.iter().map(|r| cell(r, c).chars().count()).chain(std::iter::once(c.len())).max().unwrap_or(0)).collect();
    let header: Vec<String> = columns.iter().zip(&widths).map(|(c, w)| format!("{:<w$}", c.to_uppercase(), w = *w)).collect();
    writeln!(out, "{}", bold(&header.join("  ")))?;
    for row in &rows {
        let line: Vec<String> = columns.iter().zip(&widths).map(|(c, w)| format!("{:<w$}", cell(row, c), w = *w)).collect();
        writeln!(out, "{}", line.join("  "))?;
    }
    out.flush()
}

pub fn plain(out: &mut dyn Write, value: &serde_json::Value) -> std::io::Result<()> {
    match listed(value) {
        Some((items, _)) => {
            for item in items {
                let line = match &item {
                    serde_json::Value::Object(map) => map.values().filter(|v| !v.is_object() && !v.is_array()).map(scalar).collect::<Vec<_>>().join("\t"),
                    other => scalar(other),
                };
                writeln!(out, "{}", line)?;
            }
        }
        None => writeln!(out, "{}", scalar(value))?,
    }
    out.flush()
}

pub fn template(source: &str, value: &serde_json::Value) -> Result<String, String> {
    let source = match source.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{} could not be read: {}", path, e))?,
        None => source.to_string(),
    };
    let mut env = minijinja::Environment::new();
    env.add_filter("json", |v: minijinja::Value| serde_json::to_string(&v).unwrap_or_default());
    env.add_filter("pluck", |v: minijinja::Value, key: String| -> Vec<minijinja::Value> { v.try_iter().map(|items| items.map(|item| item.get_attr(&key).unwrap_or_default()).collect()).unwrap_or_default() });
    env.add_filter("timeago", |v: String| -> String {
        let Ok(at) = chrono::DateTime::parse_from_rfc3339(&v) else { return v };
        let seconds = (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_seconds();
        match seconds {
            s if s < 60 => format!("{}s ago", s.max(0)),
            s if s < 3600 => format!("{}m ago", s / 60),
            s if s < 86400 => format!("{}h ago", s / 3600),
            s => format!("{}d ago", s / 86400),
        }
    });
    env.add_filter("color", |v: String, name: String| -> String {
        match name.as_str() {
            "red" => red(&v),
            "green" => green(&v),
            "yellow" => yellow(&v),
            "cyan" => cyan(&v),
            "dim" => dim(&v),
            "bold" => bold(&v),
            _ => v,
        }
    });
    env.add_template("cli", &source).map_err(|e| format!("--template does not parse: {}", e))?;
    let rendered = env.get_template("cli").map_err(|e| e.to_string())?.render(minijinja::Value::from_serialize(value)).map_err(|e| format!("--template failed: {}", e))?;
    Ok(rendered)
}

pub fn jq(expression: &str, value: &serde_json::Value) -> Result<String, String> {
    use std::io::Read;
    let mut child = std::process::Command::new("jq")
        .args(["-c", expression])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "--jq needs jq on the PATH; install it with brew install jq".to_string())?;
    child.stdin.take().ok_or("jq has no input")?.write_all(value.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let mut text = String::new();
    child.stdout.take().ok_or("jq has no output")?.read_to_string(&mut text).map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        let mut error = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_string(&mut error);
        }
        return Err(format!("--jq failed: {}", error.trim()));
    }
    Ok(text.trim_end().to_string())
}

pub fn emit(out: &mut dyn Write, value: serde_json::Value, shape_of: &Shape, terminal: bool) -> Result<(), String> {
    let shaped = shape(value, shape_of);
    let written = match (&shape_of.jq, &shape_of.template, mode(shape_of, terminal)) {
        (Some(expression), _, _) => jq(expression, &shaped.value).and_then(|text| writeln!(out, "{}", text).map_err(|e| e.to_string())),
        (None, Some(source), _) => template(source, &shaped.value).and_then(|text| write!(out, "{}", text).map_err(|e| e.to_string())),
        (None, None, Mode::Raw) => json(out, &shaped.value, true).map_err(|e| e.to_string()),
        (None, None, Mode::Json) => json(out, &shaped.value, false).map_err(|e| e.to_string()),
        (None, None, Mode::Plain) => plain(out, &shaped.value).map_err(|e| e.to_string()),
        (None, None, Mode::Table) => table(out, &shaped.value).map_err(|e| e.to_string()),
    };
    written?;
    if let Some((shown, total)) = shaped.truncated {
        eprintln!("{}", dim(&format!("showing {} of {}; continue with --cursor {}, or narrow with --query key=value or --fields", shown, total, shown)));
    }
    Ok(())
}
