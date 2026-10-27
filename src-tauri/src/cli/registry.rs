use serde_json::{json, Map, Value};

pub struct ParamSpec {
    pub key: &'static str,
    pub rust: &'static str,
    pub schema: &'static str,
    pub required: bool,
}

pub struct CommandSpec {
    pub name: &'static str,
    pub doc: &'static str,
    pub params: &'static [ParamSpec],
}

include!(concat!(env!("OUT_DIR"), "/commands.rs"));

#[derive(Debug, PartialEq, Eq)]
pub struct Route {
    pub resource: String,
    pub verb: Option<String>,
}

pub const VERBS: [(&str, &str); 4] = [("get", "get"), ("post", "create"), ("put", "update"), ("delete", "delete")];
const OUTBOUND: [&str; 7] = ["post_message", "post_message_reply", "post_message_forward", "post_thread_reply", "post_content_send", "post_message_reaction", "post_chat_message"];

pub fn route(name: &str) -> Route {
    match name.split_once('_').and_then(|(verb, rest)| VERBS.iter().find(|(rust, _)| *rust == verb).map(|(_, cli)| (*cli, rest))) {
        Some((verb, rest)) => Route { resource: rest.replace('_', "-"), verb: Some(verb.to_string()) },
        None => Route { resource: name.replace('_', "-"), verb: None },
    }
}

pub fn tool_name(spec: &CommandSpec) -> String {
    let route = route(spec.name);
    match route.verb {
        Some(verb) => format!("{}_{}", route.resource.replace('-', "_"), verb),
        None => route.resource.replace('-', "_"),
    }
}

pub fn find_tool(name: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|spec| tool_name(spec) == name)
}

pub fn destructive(spec: &CommandSpec) -> bool {
    route(spec.name).verb.as_deref() == Some("delete")
}

pub fn reaches_people(spec: &CommandSpec) -> bool {
    OUTBOUND.contains(&spec.name)
}

pub fn idempotent(spec: &CommandSpec) -> bool {
    matches!(route(spec.name).verb.as_deref(), Some("get" | "update" | "delete"))
}

pub fn find(resource: &str, verb: Option<&str>) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|spec| {
        let r = route(spec.name);
        r.resource == resource && r.verb.as_deref() == verb
    })
}

pub fn kebab(key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        if c.is_uppercase() {
            out.push('-');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub fn schema(param: &ParamSpec) -> Value {
    serde_json::from_str(param.schema).unwrap_or_else(|_| json!({}))
}

fn is_string(param: &ParamSpec) -> bool {
    schema(param).get("type").and_then(Value::as_str) == Some("string")
}

pub fn flag_value(param: &ParamSpec, raw: &str) -> Value {
    if is_string(param) {
        return Value::String(raw.to_string());
    }
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

pub fn put_dotted(object: &mut Map<String, Value>, path: &str, value: Value) {
    match path.split_once('.') {
        None => {
            object.insert(path.to_string(), value);
        }
        Some((head, rest)) => {
            let entry = object.entry(head.to_string()).or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            if let Value::Object(inner) = entry {
                put_dotted(inner, rest, value);
            }
        }
    }
}

pub fn arguments(spec: &CommandSpec, data: Option<Value>, flags: &[(&ParamSpec, String)], sets: &[String]) -> Result<Value, String> {
    let mut object = match data {
        Some(Value::Object(map)) => map,
        Some(Value::Null) | None => Map::new(),
        Some(other) => return Err(format!("--data must be a JSON object, got {}", other)),
    };
    for set in sets {
        let (path, raw) = set.split_once('=').ok_or_else(|| format!("--set {} needs key=value", set))?;
        let value = serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()));
        put_dotted(&mut object, path.trim(), value);
    }
    for (param, raw) in flags {
        object.insert(param.key.to_string(), flag_value(param, raw));
    }
    let missing: Vec<String> = spec.params.iter().filter(|p| p.required && !object.contains_key(p.key)).map(|p| format!("--{}", kebab(p.key))).collect();
    if !missing.is_empty() {
        return Err(format!("missing {}", missing.join(", ")));
    }
    Ok(Value::Object(object))
}

pub fn input_schema(spec: &CommandSpec) -> Value {
    let properties: Map<String, Value> = spec.params.iter().map(|p| (p.key.to_string(), schema(p))).collect();
    let required: Vec<&str> = spec.params.iter().filter(|p| p.required).map(|p| p.key).collect();
    json!({ "type": "object", "properties": properties, "required": required })
}

pub fn summary(spec: &CommandSpec) -> String {
    if !spec.doc.is_empty() {
        return spec.doc.to_string();
    }
    let route = route(spec.name);
    match route.verb.as_deref() {
        Some("get") => format!("Read {}", route.resource.replace('-', " ")),
        Some("create") => format!("Create {}", route.resource.replace('-', " ")),
        Some("update") => format!("Change {}", route.resource.replace('-', " ")),
        Some("delete") => format!("Remove {}", route.resource.replace('-', " ")),
        Some(verb) => format!("{} {}", verb, route.resource.replace('-', " ")),
        None => route.resource,
    }
}

pub fn reads_only(spec: &CommandSpec) -> bool {
    route(spec.name).verb.as_deref() == Some("get")
}

pub fn example(spec: &CommandSpec) -> String {
    let route = route(spec.name);
    let words = match &route.verb {
        Some(verb) => format!("chatnip {} {}", route.resource, verb),
        None => format!("chatnip {}", route.resource),
    };
    let flags: Vec<String> = spec.params.iter().filter(|p| p.required).map(|p| format!("--{} <{}>", kebab(p.key), kebab(p.key))).collect();
    if flags.is_empty() { words } else { format!("{} {}", words, flags.join(" ")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_into_resource_and_the_formula_verb() {
        assert_eq!(route("get_conversations"), Route { resource: "conversations".into(), verb: Some("get".into()) });
        assert_eq!(route("post_event"), Route { resource: "event".into(), verb: Some("create".into()) });
        assert_eq!(route("put_conversation_read"), Route { resource: "conversation-read".into(), verb: Some("update".into()) });
        assert_eq!(route("delete_person_identity"), Route { resource: "person-identity".into(), verb: Some("delete".into()) });
        assert_eq!(route("post_message"), Route { resource: "message".into(), verb: Some("create".into()) });
    }

    #[test]
    fn every_registered_command_starts_with_a_rust_verb_and_maps_to_a_formula_verb() {
        for spec in COMMANDS {
            assert!(VERBS.iter().any(|(rust, _)| spec.name.starts_with(&format!("{}_", rust))), "{} does not start with get, post, put or delete", spec.name);
            assert!(route(spec.name).verb.is_some(), "{} has no verb", spec.name);
        }
        let send = find("message", Some("create")).unwrap();
        assert_eq!(tool_name(send), "message_create");
        assert!(reaches_people(send) && !destructive(send));
        assert!(destructive(find("draft", Some("delete")).unwrap()));
    }

    #[test]
    fn the_registry_holds_every_registered_command_once() {
        assert!(COMMANDS.len() > 140);
        let conversations = find("conversations", Some("get")).unwrap();
        assert_eq!(conversations.name, "get_conversations");
        let summary = find("action-code", Some("get")).unwrap();
        assert_eq!(summary.params[0].key, "actionId");
        let event = find("event", Some("create")).unwrap();
        assert!(event.params.iter().any(|p| p.key == "calendarIds" || p.key == "input" || p.key == "draft"));
        let mut names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), COMMANDS.len());
        assert!(COMMANDS.iter().all(|c| c.params.iter().all(|p| !p.key.contains('_'))));
    }

    #[test]
    fn no_approval_command_is_registered() {
        let approvals: Vec<&str> = COMMANDS.iter().map(|c| c.name).filter(|n| n.contains("approval")).collect();
        assert!(approvals.is_empty(), "{:?}", approvals);
    }

    #[test]
    fn routes_never_clash_with_each_other_or_with_tool_commands() {
        let mut seen = std::collections::HashSet::new();
        for spec in COMMANDS {
            let r = route(spec.name);
            assert!(seen.insert((r.resource.clone(), r.verb.clone())), "{} clashes", spec.name);
            assert!(!crate::cli::TOOLS.contains(&r.resource.as_str()), "{} clashes with a tool command", spec.name);
        }
    }

    #[test]
    fn flags_become_json_arguments() {
        let spec = find("action-code", Some("get")).unwrap();
        let args = arguments(spec, None, &[(&spec.params[0], "42".into())], &[]).unwrap();
        assert_eq!(args, json!({ "actionId": "42" }));
        assert_eq!(kebab("conversationId"), "conversation-id");
        let int = ParamSpec { key: "limit", rust: "u32", schema: r#"{"type":"integer"}"#, required: false };
        assert_eq!(flag_value(&int, "20"), json!(20));
        let list = ParamSpec { key: "ids", rust: "Vec<String>", schema: r#"{"type":"array","items":{"type":"string"}}"#, required: true };
        assert_eq!(flag_value(&list, r#"["a","b"]"#), json!(["a", "b"]));
        assert_eq!(flag_value(&list, "plain"), json!("plain"));
        assert!(arguments(spec, Some(json!({})), &[], &[]).unwrap_err().contains("--action-id"));
        assert_eq!(arguments(spec, Some(json!({ "actionId": "7" })), &[], &[]).unwrap(), json!({ "actionId": "7" }));
    }

    #[test]
    fn set_flags_nest_by_dots_and_read_json_values() {
        let spec = find("action-code", Some("get")).unwrap();
        let args = arguments(spec, None, &[], &["actionId=7".into(), "input.limit=20".into(), "input.name=Anna".into(), "input.flags=[1,2]".into()]).unwrap();
        assert_eq!(args, json!({ "actionId": 7, "input": { "limit": 20, "name": "Anna", "flags": [1, 2] } }));
        assert!(arguments(spec, None, &[], &["nothing".into()]).unwrap_err().contains("key=value"));
    }

    #[test]
    fn tool_schemas_list_properties_and_required_keys() {
        let spec = find("action-code", Some("get")).unwrap();
        let schema = input_schema(spec);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["actionId"]["type"], "string");
        assert_eq!(schema["required"], json!(["actionId"]));
    }
}
