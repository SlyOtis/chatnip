use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const INJECTED: [&str; 6] = ["State", "Window", "WebviewWindow", "Webview", "AppHandle", "Request"];

struct Param {
    key: String,
    rust: String,
    schema: String,
    required: bool,
}

struct Field {
    key: String,
    ty: syn::Type,
    optional: bool,
}

type Structs = HashMap<String, Vec<Field>>;

struct Command {
    name: String,
    doc: String,
    params: Vec<Param>,
}

fn main() {
    let src = Path::new("src");
    println!("cargo:rerun-if-changed=src");
    let mut structs: Structs = HashMap::new();
    collect_structs(src, &mut structs);
    let registered = registered_commands(&fs::read_to_string(src.join("main.rs")).expect("src/main.rs"));
    let mut found: HashMap<(String, String), Command> = HashMap::new();
    for entry in fs::read_dir(src).expect("src").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let module = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let text = fs::read_to_string(&path).expect("source file");
        let Ok(file) = syn::parse_file(&text) else { continue };
        for item in file.items {
            if let syn::Item::Fn(function) = item {
                if is_command(&function.attrs) {
                    let command = describe(&function, &structs);
                    found.insert((module.clone(), command.name.clone()), command);
                }
            }
        }
    }
    let mut out = String::from("pub static COMMANDS: &[CommandSpec] = &[\n");
    for (module, name) in registered {
        let key = (module.unwrap_or_else(|| "commands".to_string()), name.clone());
        let command = found.remove(&key).or_else(|| {
            let other = found.keys().find(|(_, n)| *n == name).cloned();
            other.and_then(|k| found.remove(&k))
        });
        let Some(command) = command else {
            panic!("command {} is registered but its #[tauri::command] function was not found", name)
        };
        write!(out, "    CommandSpec {{ name: {:?}, doc: {:?}, params: &[", command.name, command.doc).unwrap();
        for param in &command.params {
            write!(out, "ParamSpec {{ key: {:?}, rust: {:?}, schema: {:?}, required: {} }}, ", param.key, param.rust, param.schema, param.required).unwrap();
        }
        out.push_str("] },\n");
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("commands.rs");
    fs::write(dest, out).expect("write command registry");
    emoji_table();
    tauri_build::build()
}

const EMOJI_DATA: &str = "../node_modules/emojibase-data/en";
const EMOJI_PRESETS: [&str; 3] = ["iamcal", "github", "emojibase"];

fn emoji_json(name: &str) -> serde_json::Value {
    let path = Path::new(EMOJI_DATA).join(name);
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is missing ({}); run pnpm install in the repository root", path.display(), e));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {}", path.display(), e))
}

fn emoji_unicode(hexcode: &str) -> String {
    hexcode.split('-').filter_map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32)).collect()
}

fn emoji_tone(tone: &serde_json::Value) -> Option<String> {
    let tones: Vec<u64> = match tone {
        serde_json::Value::Number(n) => vec![n.as_u64()?],
        serde_json::Value::Array(list) => list.iter().filter_map(|t| t.as_u64()).collect(),
        _ => return None,
    };
    let mut distinct: Vec<u64> = Vec::new();
    for t in tones {
        if !distinct.contains(&t) {
            distinct.push(t);
        }
    }
    Some(format!("skin-tone-{}", distinct.iter().map(|t| (t + 1).to_string()).collect::<Vec<_>>().join("-")))
}

fn emoji_table() {
    let data = emoji_json("data.json");
    let presets: Vec<serde_json::Value> = EMOJI_PRESETS.iter().map(|p| emoji_json(&format!("shortcodes/{}.json", p))).collect();
    let names_of = |hexcode: &str| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for preset in &presets {
            let found = match &preset[hexcode] {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(list) => list.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
                _ => Vec::new(),
            };
            for name in found {
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        out
    };
    let mut rows: Vec<(String, String, bool, bool)> = Vec::new();
    for entry in data.as_array().expect("emojibase data is a list") {
        let Some(hexcode) = entry["hexcode"].as_str() else { continue };
        let names = names_of(hexcode);
        if names.is_empty() {
            continue;
        }
        let unicode = emoji_unicode(hexcode);
        let text = entry["type"].as_u64() == Some(0);
        for (i, name) in names.iter().enumerate() {
            rows.push((name.clone(), unicode.clone(), i == 0, text));
        }
        for skin in entry["skins"].as_array().cloned().unwrap_or_default() {
            let (Some(skin_hex), Some(tone)) = (skin["hexcode"].as_str(), emoji_tone(&skin["tone"])) else { continue };
            let skin_unicode = emoji_unicode(skin_hex);
            for (i, name) in names.iter().enumerate() {
                rows.push((format!("{}::{}", name, tone), skin_unicode.clone(), i == 0, false));
            }
        }
    }
    let mut out = String::from("pub static EMOJI: &[(&str, &str, bool, bool)] = &[\n");
    for (name, unicode, primary, text) in rows {
        writeln!(out, "    ({:?}, {:?}, {}, {}),", name, unicode, primary, text).unwrap();
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("emoji.rs");
    fs::write(dest, out).expect("write emoji table");
}

fn registered_commands(main: &str) -> Vec<(Option<String>, String)> {
    let start = main.find("generate_handler![").expect("generate_handler! in main.rs") + "generate_handler![".len();
    let end = start + main[start..].find(']').expect("end of generate_handler!");
    main[start..end]
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|path| {
            let mut parts: Vec<&str> = path.split("::").collect();
            let name = parts.pop().unwrap_or_default().to_string();
            (parts.last().map(|m| m.to_string()), name)
        })
        .collect()
}

fn is_command(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let segments: Vec<String> = attr.path().segments.iter().map(|s| s.ident.to_string()).collect();
        segments == ["tauri", "command"]
    })
}

fn collect_structs(dir: &Path, structs: &mut Structs) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_structs(&path, structs);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let Ok(file) = syn::parse_file(&text) else { continue };
        for item in file.items {
            let syn::Item::Struct(item) = item else { continue };
            let syn::Fields::Named(named) = &item.fields else { continue };
            let camel_all = serde_values(&item.attrs).iter().any(|(k, v)| k == "rename_all" && v == "camelCase");
            let mut fields = Vec::new();
            for field in &named.named {
                let Some(ident) = &field.ident else { continue };
                let serde = serde_values(&field.attrs);
                if serde.iter().any(|(k, _)| k == "skip" || k == "skip_deserializing") {
                    continue;
                }
                let renamed = serde.iter().find(|(k, _)| k == "rename").map(|(_, v)| v.clone());
                let raw = ident.to_string();
                let key = renamed.unwrap_or_else(|| if camel_all { camel(&raw) } else { raw });
                let optional = last_ident(&field.ty) == "Option" || serde.iter().any(|(k, _)| k == "default");
                fields.push(Field { key, ty: field.ty.clone(), optional });
            }
            structs.entry(item.ident.to_string()).or_insert(fields);
        }
    }
}

fn serde_values(attrs: &[syn::Attribute]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            let value = match meta.value() {
                Ok(stream) => stream.parse::<syn::LitStr>().map(|s| s.value()).unwrap_or_default(),
                Err(_) => String::new(),
            };
            out.push((key, value));
            Ok(())
        });
    }
    out
}

fn describe(function: &syn::ItemFn, structs: &Structs) -> Command {
    let doc = function
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut params = Vec::new();
    for input in &function.sig.inputs {
        let syn::FnArg::Typed(typed) = input else { continue };
        let syn::Pat::Ident(ident) = typed.pat.as_ref() else { continue };
        if INJECTED.contains(&last_ident(&typed.ty).as_str()) {
            continue;
        }
        let (schema, optional) = schema_of(&typed.ty, structs, 0);
        params.push(Param {
            key: camel(ident.ident.to_string().trim_start_matches('_')),
            rust: type_text(&typed.ty),
            schema,
            required: !optional,
        });
    }
    Command { name: function.sig.ident.to_string(), doc, params }
}

fn last_ident(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
        syn::Type::Reference(reference) => last_ident(&reference.elem),
        _ => String::new(),
    }
}

fn generic_args(ty: &syn::Type) -> Vec<syn::Type> {
    let syn::Type::Path(path) = ty else { return Vec::new() };
    let Some(last) = path.path.segments.last() else { return Vec::new() };
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else { return Vec::new() };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn schema_of(ty: &syn::Type, structs: &Structs, depth: usize) -> (String, bool) {
    let name = last_ident(ty);
    let inner = generic_args(ty);
    match name.as_str() {
        "Option" => (inner.first().map(|t| schema_of(t, structs, depth).0).unwrap_or_else(|| "{}".into()), true),
        "String" | "str" => (r#"{"type":"string"}"#.into(), false),
        "bool" => (r#"{"type":"boolean"}"#.into(), false),
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => (r#"{"type":"integer"}"#.into(), false),
        "f32" | "f64" => (r#"{"type":"number"}"#.into(), false),
        "Vec" => {
            let items = inner.first().map(|t| schema_of(t, structs, depth).0).unwrap_or_else(|| "{}".into());
            (format!(r#"{{"type":"array","items":{}}}"#, items), false)
        }
        "HashMap" | "BTreeMap" | "Map" => (r#"{"type":"object"}"#.into(), false),
        "Value" => ("{}".into(), false),
        other => match structs.get(other) {
            Some(fields) if depth < 3 => {
                let properties: Vec<String> = fields.iter().map(|f| format!("{:?}:{}", f.key, schema_of(&f.ty, structs, depth + 1).0)).collect();
                let required: Vec<String> = fields.iter().filter(|f| !f.optional).map(|f| format!("{:?}", f.key)).collect();
                (format!(r#"{{"type":"object","description":"{}","properties":{{{}}},"required":[{}]}}"#, other, properties.join(","), required.join(",")), false)
            }
            _ => (format!(r#"{{"type":"object","description":"{}"}}"#, other), false),
        },
    }
}

fn type_text(ty: &syn::Type) -> String {
    let name = last_ident(ty);
    let inner: Vec<String> = generic_args(ty).iter().map(type_text).collect();
    if inner.is_empty() {
        name
    } else {
        format!("{}<{}>", name, inner.join(", "))
    }
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
