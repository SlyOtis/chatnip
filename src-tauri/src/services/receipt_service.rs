use crate::models::action::AiChoice;
use crate::models::app_setting::{self, AppSetting};
use crate::models::intelligence::{IntelligenceConnection, KIND_OLLAMA};
use crate::models::Attachment;
use crate::services::{attachment_service, intelligence_service};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use crate::utils::scalar_text;
use serde_json::{json, Map, Value};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;

pub const TEXT_LIMIT: usize = 12_000;
pub const PROMPT_TEXT: usize = 1_500;
pub const EXCERPT: usize = 280;
pub const PROMPT_KEY: &str = "receipt_confirm";
pub const METHOD_HTML: &str = "html";
pub const METHOD_PDF: &str = "pdf";
pub const METHOD_OCR: &str = "ocr";
pub const METHOD_TEXT: &str = "text";
pub const METHOD_SKIPPED: &str = "skipped";
pub const FIELD_SUBJECT: &str = "subject";
pub const FIELD_BODY: &str = "body";
pub const FIELD_FILE: &str = "file";
pub const FIELD_SENDER: &str = "sender";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub pattern: String,
    pub weight: f64,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hit {
    pub pattern: String,
    pub matched: String,
    pub source: String,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Extracted {
    pub method: &'static str,
    pub text: String,
    pub error: Option<String>,
}

fn pattern(name: &str, source: &str, weight: f64, fields: &[&str]) -> Pattern {
    Pattern { name: name.into(), pattern: source.into(), weight, fields: fields.iter().map(|f| f.to_string()).collect() }
}

pub fn default_patterns() -> Vec<Pattern> {
    vec![
        pattern("receipt_word", r"\b(kvittering|receipt|ordrebekreftelse|order confirmation|kjøpsbekreftelse|purchase confirmation|betalingsbekreftelse|payment confirmation|takk for (handelen|kjøpet|bestillingen|ordren)|thank you for your (order|purchase|payment))\b", 0.45, &[]),
        pattern("paid_invoice", r"\b(faktura|invoice)\b[^\n]{0,80}\b(er betalt|betalt|paid|settled)\b|\b(betalt|paid)\b[^\n]{0,40}\b(faktura|invoice)\b", 0.4, &[]),
        pattern("total", r"\b(totalt|total|sum|å betale|betalt beløp|amount paid|amount charged|grand total|order total|totalbeløp|beløp)\b[^\d\n]{0,25}\d", 0.25, &[]),
        pattern("vat", r"\b(mva|moms|vat|merverdiavgift|sales tax)\b", 0.15, &[]),
        pattern("org_number", r"\borg\.?\s*nr\.?\s*:?\s*(no\s*)?\d{3}\s?\d{3}\s?\d{3}\b", 0.15, &[]),
        pattern("amount", r"\b(kr|nok|eur|usd|sek)\.?\s?\d[\d\s.,]*\d|\d[\d\s.,]*\d\s?(kr|nok|eur|usd|sek)\b|[€$£]\s?\d[\d.,]*\d|\d+,-", 0.15, &[]),
        pattern("card_ending", r"(\*{2,}|x{3,}|•{2,}|ending in|slutter på|kortnummer\s*:?)\s*\d{4}\b|\b(visa|mastercard|amex|american express)\b[^\n\d]{0,20}\d{4}\b|\bbetalt med vipps\b|\bpaid with (apple pay|google pay|paypal)\b", 0.15, &[]),
        pattern("order_number", r"\b(ordre(nummer|nr\.?)?|order\s?(no\.?|number|#|id)|bestillingsnummer|kvitteringsnummer|receipt\s?(no\.?|number|#)|transaksjons?(id|nr\.?|nummer))\s*[:#.]?\s*[a-z]*\d[a-z0-9-]{2,}", 0.15, &[]),
        pattern("store_sender", r"\b(vipps|elkjøp|elkjop|komplett|power|xxl|ikea|clas ohlson|apple|amazon|zalando|foodora|wolt|uber|sas|norwegian|ruter|vy|rema|kiwi|coop|meny|apotek ?1|vinmonopolet|spotify|netflix|steam|paypal|stripe|ebay|finn|nordlys elektro)\b|\b(kvittering|receipts?|orders?|ordre|billing|faktura)@", 0.1, &[FIELD_SENDER]),
        pattern("newsletter", r"\b(unsubscribe|avslutt abonnement|meld deg av|avmeld|nyhetsbrev|newsletter|ukens tilbud|kampanje|rabattkode|discount code|black friday|price drop|tilbudet gjelder)\b|\d+\s?% (off|rabatt)", -0.35, &[]),
        pattern("unpaid", r"\b(forfallsdato|forfaller|due date|betal innen|pay by|ubetalt|unpaid|purring|payment reminder|kid-?nummer|kid:)", -0.3, &[]),
        pattern("receipt_of", r"\b(confirm(ing)? (the )?receipt of|bekrefter mottak|mottatt din (søknad|henvendelse)|we have received your (application|request))\b", -0.4, &[]),
    ]
}

fn compiled(source: &str) -> Result<Regex, String> {
    RegexBuilder::new(source).case_insensitive(true).size_limit(1 << 21).build().map_err(|e| format!("Bad pattern {}: {}", source, e))
}

pub fn patterns_from(value: &Value) -> Result<Vec<Pattern>, String> {
    match value {
        Value::Null => Ok(default_patterns()),
        Value::Array(items) if items.is_empty() => Ok(default_patterns()),
        other => serde_json::from_value(other.clone()).map_err(|e| format!("The patterns are not a list of name, pattern and weight: {}", e)),
    }
}

pub fn hits(patterns: &[Pattern], texts: &[(&str, &str)]) -> Result<Vec<Hit>, String> {
    let mut out = Vec::new();
    for p in patterns {
        let re = compiled(&p.pattern)?;
        let found = texts.iter().filter(|(field, _)| if p.fields.is_empty() { *field != FIELD_SENDER } else { p.fields.iter().any(|f| f == field) }).find_map(|(field, text)| re.find(text).map(|m| (field, m.as_str().trim().to_string())));
        if let Some((field, matched)) = found {
            out.push(Hit { pattern: p.name.clone(), matched: matched.chars().take(120).collect(), source: field.to_string(), weight: p.weight });
        }
    }
    Ok(out)
}

pub fn score(hits: &[Hit]) -> f64 {
    let sum: f64 = hits.iter().map(|h| h.weight).sum();
    (sum.clamp(0.0, 1.0) * 100.0).round() / 100.0
}


fn capped(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

pub fn file_kind(name: &str, content_type: &str) -> &'static str {
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if content_type == "application/pdf" || ext == "pdf" {
        METHOD_PDF
    } else if content_type.contains("html") || ext == "html" || ext == "htm" {
        METHOD_HTML
    } else if content_type.starts_with("image/") || ["png", "jpg", "jpeg", "heic", "tiff", "tif", "gif", "bmp", "webp"].contains(&ext) {
        METHOD_OCR
    } else if content_type.starts_with("text/") || ["txt", "csv", "md", "eml"].contains(&ext) {
        METHOD_TEXT
    } else {
        METHOD_SKIPPED
    }
}

pub fn html_text(html: &str) -> String {
    crate::content::html::visible_text(html)
}

pub fn pdf_text(bytes: &[u8]) -> Result<String, String> {
    let document = lopdf::Document::load_mem(bytes).map_err(|e| format!("The PDF could not be read: {}", e))?;
    let pages: Vec<u32> = document.get_pages().keys().copied().collect();
    if pages.is_empty() {
        return Err("The PDF has no pages".to_string());
    }
    let text = document.extract_text(&pages).map_err(|e| format!("The PDF text could not be extracted: {}", e))?;
    Ok(text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n"))
}

#[cfg(target_os = "macos")]
pub fn ocr(bytes: &[u8]) -> Result<String, String> {
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};
    objc2::rc::autoreleasepool(|_| {
        let data = NSData::with_bytes(bytes);
        let options = NSDictionary::new();
        let handler = VNImageRequestHandler::initWithData_options(VNImageRequestHandler::alloc(), &data, &options);
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        let supported: Vec<String> = unsafe { request.supportedRecognitionLanguagesAndReturnError() }.map(|list| list.iter().map(|l| l.to_string()).collect()).unwrap_or_default();
        let wanted: Vec<_> = ["nb-NO", "da-DK", "sv-SE", "en-US"].iter().filter(|l| supported.iter().any(|s| s == *l)).map(|l| NSString::from_str(l)).collect();
        if !wanted.is_empty() {
            request.setRecognitionLanguages(&NSArray::from_retained_slice(&wanted));
        }
        let generic: &VNRequest = &request;
        handler.performRequests_error(&NSArray::from_slice(&[generic])).map_err(|e| format!("Vision could not read the image: {}", e.localizedDescription()))?;
        let mut lines = Vec::new();
        if let Some(results) = request.results() {
            for observation in results.iter() {
                if let Some(best) = observation.topCandidates(1).iter().next() {
                    lines.push(best.string().to_string());
                }
            }
        }
        Ok(lines.join("\n"))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn ocr(_bytes: &[u8]) -> Result<String, String> {
    Err("Reading text in images needs macOS Vision".to_string())
}

pub fn extract_bytes(name: &str, content_type: &str, bytes: &[u8]) -> Extracted {
    let method = file_kind(name, content_type);
    let outcome = match method {
        METHOD_PDF => pdf_text(bytes),
        METHOD_HTML => Ok(html_text(&String::from_utf8_lossy(bytes))),
        METHOD_OCR => ocr(bytes),
        METHOD_TEXT => Ok(String::from_utf8_lossy(bytes).to_string()),
        _ => Err(format!("{} is not a kind of file Chatnip reads", if content_type.is_empty() { name } else { content_type })),
    };
    match outcome {
        Ok(text) => Extracted { method, text: capped(&text, TEXT_LIMIT), error: None },
        Err(error) => Extracted { method: if method == METHOD_SKIPPED { METHOD_SKIPPED } else { method }, text: String::new(), error: Some(error) },
    }
}

pub fn body_text(content: &str, content_type: &str) -> String {
    let text = if matches!(content_type, "html" | "text/html") { html_text(content) } else { content.to_string() };
    capped(&text, TEXT_LIMIT)
}

pub async fn material(pool: &Pool<Sqlite>, app_data: &Path, cache: &Path, message_id: &str) -> Result<Value, String> {
    let row = sqlx::query(
        "SELECT m.id, m.conversation_id, m.subject, m.content, m.content_type, m.sender_name, m.sender_email, m.timestamp, m.attachments,
            c.name AS conversation_name, p.provider_type, p.name AS connection
         FROM message m LEFT JOIN conversation c ON c.id = m.conversation_id LEFT JOIN provider p ON p.id = m.provider_id WHERE m.id = ?1",
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to read the message: {}", e))?
    .ok_or_else(|| format!("No message with id {}", message_id))?;
    let optional = |column: &str| row.try_get::<Option<String>, _>(column).ok().flatten().unwrap_or_default();
    let attachments: Vec<Attachment> = crate::models::Message::parse_attachments(row.try_get::<Option<String>, _>("attachments").ok().flatten().as_deref());
    let mut files = Vec::new();
    for (index, attachment) in attachments.iter().enumerate().filter(|(_, a)| !a.is_inline) {
        let method = file_kind(&attachment.name, &attachment.content_type);
        let mut entry = json!({ "id": attachment.id, "index": index, "name": attachment.name, "content_type": attachment.content_type, "size": attachment.size, "path": null, "method": method, "chars": 0, "text": "", "error": null });
        if method == METHOD_SKIPPED {
            entry["error"] = json!(format!("{} is not a kind of file Chatnip reads", attachment.content_type));
            files.push(entry);
            continue;
        }
        match attachment_service::cached(pool, app_data, cache, message_id, attachment).await {
            Ok(path) => {
                entry["path"] = json!(path.to_string_lossy());
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let extracted = extract_bytes(&attachment.name, &attachment.content_type, &bytes);
                        entry["method"] = json!(extracted.method);
                        entry["chars"] = json!(extracted.text.chars().count());
                        entry["text"] = json!(extracted.text);
                        entry["error"] = json!(extracted.error);
                    }
                    Err(e) => entry["error"] = json!(format!("The cached file could not be read: {}", e)),
                }
            }
            Err(e) => entry["error"] = json!(e),
        }
        files.push(entry);
    }
    Ok(json!({
        "message_id": message_id,
        "conversation_id": optional("conversation_id"),
        "conversation_name": optional("conversation_name"),
        "provider": optional("provider_type"),
        "connection": optional("connection"),
        "subject": optional("subject"),
        "sender": optional("sender_name"),
        "sender_email": optional("sender_email"),
        "at": optional("timestamp"),
        "body": body_text(&optional("content"), &optional("content_type")),
        "files": files,
    }))
}

fn base_candidate(material: &Value, id: String, source: &str, file: Value, text: &str, score: f64, hits: Vec<Hit>) -> Value {
    json!({
        "id": id,
        "source": source,
        "message_id": material["message_id"],
        "conversation_id": material["conversation_id"],
        "conversation_name": material["conversation_name"],
        "provider": material["provider"],
        "connection": material["connection"],
        "subject": material["subject"],
        "sender": material["sender"],
        "sender_email": material["sender_email"],
        "at": material["at"],
        "file": file,
        "text": text,
        "score": score,
        "hits": hits,
        "confirmed": null,
    })
}

const EVIDENCE: [&str; 6] = ["receipt_word", "paid_invoice", "total", "vat", "org_number", "amount"];

pub fn candidates(material: &Value, patterns: &[Pattern], threshold: f64) -> Result<Vec<Value>, String> {
    let subject = scalar_text(material.get("subject"));
    let sender = format!("{} {}", scalar_text(material.get("sender")), scalar_text(material.get("sender_email")));
    let body = scalar_text(material.get("body"));
    let message_id = scalar_text(material.get("message_id"));
    let mut out = Vec::new();
    for file in material["files"].as_array().cloned().unwrap_or_default() {
        let text = scalar_text(file.get("text"));
        if text.trim().is_empty() {
            continue;
        }
        let own = hits(patterns, &[(FIELD_FILE, &text)])?;
        if !own.iter().any(|h| h.weight > 0.0 && EVIDENCE.contains(&h.pattern.as_str())) {
            continue;
        }
        let all = hits(patterns, &[(FIELD_FILE, &text), (FIELD_SUBJECT, &subject), (FIELD_BODY, &body), (FIELD_SENDER, &sender)])?;
        let s = score(&all);
        if s >= threshold {
            let id = format!("{}#file{}", message_id, file.get("index").and_then(Value::as_u64).unwrap_or(0));
            let mut entry = file.clone();
            if let Some(o) = entry.as_object_mut() {
                o.remove("text");
            }
            out.push(base_candidate(material, id, FIELD_FILE, entry, &text, s, all));
        }
    }
    if out.is_empty() {
        let all = hits(patterns, &[(FIELD_SUBJECT, &subject), (FIELD_BODY, &body), (FIELD_SENDER, &sender)])?;
        let s = score(&all);
        if s >= threshold {
            out.push(base_candidate(material, format!("{}#body", message_id), FIELD_BODY, Value::Null, &format!("{}\n{}", subject, body), s, all));
        }
    }
    Ok(out)
}

pub fn render_prompt(batch: &[Value], overrides: &Value) -> Result<(String, String, Option<Value>), String> {
    let spec = intelligence_service::prompt(PROMPT_KEY, overrides)?;
    let shown: Vec<Value> = batch.iter().map(|c| json!({ "id": c["id"], "sender": format!("{} <{}>", scalar_text(c.get("sender")), scalar_text(c.get("sender_email"))), "subject": c["subject"], "text": capped(&scalar_text(c.get("text")), PROMPT_TEXT) })).collect();
    let context = json!({ "candidates": shown, "candidates_json": serde_json::to_string(&shown).unwrap_or_default() });
    let system = intelligence_service::render(&spec.system, &context)?;
    let prompt = intelligence_service::render(&spec.template, &context)?;
    let schema = match &spec.output_schema {
        Some(raw) => Some(serde_json::from_str::<Value>(raw).map_err(|e| format!("The receipt schema is not valid JSON: {}", e))?),
        None => None,
    };
    Ok((system, prompt, schema))
}

pub async fn local_model(pool: &Pool<Sqlite>, profile_id: &str, overrides: &Value, ai: Option<&AiChoice>) -> Result<(IntelligenceConnection, String), String> {
    let node_connection = overrides.get("connection_id").and_then(Value::as_str).filter(|id| !id.is_empty());
    let choice = intelligence_service::get_feature_choice(pool, PROMPT_KEY, ai).await?;
    let chosen = match node_connection {
        Some(id) => Some(IntelligenceConnection::get(pool, id).await?.ok_or_else(|| format!("The connection {} is gone", id))?),
        None => match &choice.connection_id {
            Some(id) => IntelligenceConnection::get(pool, id).await?,
            None => None,
        },
    };
    let chosen = match chosen {
        Some(connection) => Some(connection),
        None => IntelligenceConnection::all(pool, profile_id).await?.into_iter().find(|c| c.kind == KIND_OLLAMA),
    };
    let connection = chosen.unwrap_or_else(|| IntelligenceConnection {
        id: String::new(),
        profile_id: profile_id.to_string(),
        kind: KIND_OLLAMA.to_string(),
        name: "Ollama".to_string(),
        config: Default::default(),
        is_default: false,
        available: false,
        detail: None,
        models: Vec::new(),
        created_at: String::new(),
    });
    let feature = choice.get_model(node_connection, &connection);
    let wanted = intelligence_service::get_model(overrides.get("model").and_then(Value::as_str), feature.as_deref(), None);
    if connection.kind != KIND_OLLAMA {
        let model = intelligence_service::get_model(wanted.as_deref(), None, connection.setting("model").as_deref()).ok_or_else(|| format!("{} has no model chosen", connection.name))?;
        return Ok((connection, model));
    }
    let host = match connection.setting("host").filter(|h| !h.is_empty()) {
        Some(host) => host,
        None => AppSetting::get_value(pool, app_setting::OLLAMA_HOST).await?,
    };
    let installed = intelligence_service::ollama_models(&host).await?;
    if installed.is_empty() {
        return Err(format!("Ollama at {} has no model installed; pull a small one under Settings, AI, Ollama", host));
    }
    let model = match wanted {
        Some(model) if installed.iter().any(|m| m.name == model || m.name == format!("{}:latest", model)) => model,
        Some(model) => return Err(format!("{} is not installed in Ollama at {}", model, host)),
        None => installed.iter().min_by_key(|m| m.size_bytes).map(|m| m.name.clone()).unwrap_or_default(),
    };
    Ok((connection, model))
}

pub fn apply_verdicts(batch: &mut [Value], answer: &Value) -> usize {
    let verdicts = answer.get("verdicts").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut applied = 0;
    for candidate in batch.iter_mut() {
        let id = scalar_text(candidate.get("id"));
        let Some(verdict) = verdicts.iter().find(|v| scalar_text(v.get("id")) == id) else { continue };
        let Some(receipt) = verdict.get("receipt").and_then(Value::as_bool) else { continue };
        let Some(fields) = candidate.as_object_mut() else { continue };
        fields.insert("confirmed".to_string(), json!(receipt));
        fields.insert("ai".to_string(), verdict.clone());
        applied += 1;
    }
    applied
}

pub type Ask = std::sync::Arc<dyn Fn(&str, &Value) -> Result<Value, String> + Send + Sync>;

pub async fn confirm(pool: &Pool<Sqlite>, profile_id: &str, mut candidates: Vec<Value>, batch: usize, overrides: &Value, ai: Option<&AiChoice>, ask: Option<Ask>) -> (Vec<Value>, Vec<String>) {
    let mut log = Vec::new();
    if candidates.is_empty() {
        log.push("No candidates to confirm".to_string());
        return (candidates, log);
    }
    let size = batch.clamp(1, 20);
    let chosen = match &ask {
        Some(_) => Ok(None),
        None => local_model(pool, profile_id, overrides, ai).await.map(Some),
    };
    let chosen = match chosen {
        Ok(chosen) => chosen,
        Err(reason) => {
            log.push(format!("No local model is available ({}); {} candidates pass on unconfirmed", reason, candidates.len()));
            for fields in candidates.iter_mut().filter_map(Value::as_object_mut) {
                fields.insert("confirmed".to_string(), Value::Null);
            }
            return (candidates, log);
        }
    };
    if let Some((connection, model)) = &chosen {
        log.push(format!("Confirming {} candidates with {} on {}", candidates.len(), model, connection.name));
    }
    for (n, chunk) in candidates.chunks_mut(size).enumerate() {
        let outcome = match (&ask, &chosen) {
            (Some(ask), _) => render_prompt(chunk, overrides).and_then(|(_, prompt, _)| ask(PROMPT_KEY, &json!({ "prompt": prompt, "candidates": chunk.iter().map(|c| c["id"].clone()).collect::<Vec<_>>() }))),
            (None, Some((connection, model))) => match render_prompt(chunk, overrides) {
                Ok((system, prompt, schema)) => intelligence_service::execute(pool, connection, PROMPT_KEY, &system, &prompt, Some(model), schema.as_ref()).await.map(|run| run.output),
                Err(e) => Err(e),
            },
            (None, None) => Err("No model".to_string()),
        };
        match outcome {
            Ok(answer) => {
                let applied = apply_verdicts(chunk, &answer);
                log.push(format!("Batch {}: {} of {} candidates got a verdict", n + 1, applied, chunk.len()));
            }
            Err(e) => log.push(format!("Batch {} passes on unconfirmed: {}", n + 1, e)),
        }
    }
    (candidates, log)
}

fn amount_pattern() -> &'static str {
    r"(\d{1,3}(?:[ \u{a0}.,]\d{3})+(?:[.,]\d{1,2})?|\d+(?:[.,]\d{1,2})?)"
}

pub fn parse_amount(raw: &str) -> Option<f64> {
    let cleaned: String = raw.trim().trim_end_matches(",-").chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}').collect();
    if cleaned.is_empty() {
        return None;
    }
    let commas = cleaned.matches(',').count();
    let dots = cleaned.matches('.').count();
    let decimal = if commas > 0 && dots > 0 {
        cleaned.rfind(|ch| ch == ',' || ch == '.')
    } else if commas + dots == 1 {
        let at = cleaned.rfind(|ch| ch == ',' || ch == '.').unwrap_or(0);
        (cleaned.len() - at - 1 != 3).then_some(at)
    } else {
        None
    };
    let normal: String = cleaned.char_indices().filter_map(|(i, c)| if c.is_ascii_digit() { Some(c) } else if Some(i) == decimal { Some('.') } else { None }).collect();
    normal.parse::<f64>().ok()
}

fn regex(source: &str) -> Regex {
    RegexBuilder::new(source).case_insensitive(true).build().expect("built in receipt pattern")
}

pub fn find_total(text: &str) -> Option<(f64, usize)> {
    let re = regex(&format!(r"\b(totalt|total|grand total|order total|amount paid|amount charged|å betale|betalt beløp|totalbeløp|sum|beløp)\b(?:\s+(?:inkl\.?|incl\.?|including)\s+(?:mva|vat|moms)\.?)?[^\d\n]{{0,25}}{}", amount_pattern()));
    let rank = |word: &str| match word.to_lowercase().as_str() {
        "sum" | "beløp" => 1,
        _ => 2,
    };
    re.captures_iter(text)
        .filter_map(|c| {
            let word = c.get(1)?.as_str();
            let amount = c.get(2)?;
            parse_amount(amount.as_str()).map(|v| (rank(word), v, amount.start()))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)))
        .map(|(_, v, at)| (v, at))
}

pub fn find_vat(text: &str) -> Option<f64> {
    let re = regex(&format!(r"\b(mva|moms|vat|merverdiavgift)\b(?:\s*\(?\d{{1,2}}(?:[.,]\d+)?\s?%\)?)?[^\d\n]{{0,20}}{}", amount_pattern()));
    let before = regex(r"(eks\.?|ekskl\.?|excl\.?|excluding|inkl\.?|incl\.?|including|org\.?\s*nr\.?.{0,16})\s*$");
    let found = re.captures_iter(text).find_map(|c| {
        let start = c.get(0)?.start();
        let prefix: String = text[..start].chars().rev().take(24).collect::<Vec<_>>().into_iter().rev().collect();
        if before.is_match(&prefix) {
            return None;
        }
        parse_amount(c.get(2)?.as_str())
    });
    found
}

pub fn find_currency(text: &str, near: Option<usize>) -> Option<String> {
    let code = |found: &str| match found.to_lowercase().as_str() {
        "kr" | "nok" | ",-" => "NOK",
        "€" | "eur" => "EUR",
        "$" | "usd" => "USD",
        "£" | "gbp" => "GBP",
        "sek" => "SEK",
        _ => "",
    };
    let re = regex(r"\b(nok|eur|usd|sek|gbp|kr)\b|[€$£]|,-");
    if let Some(at) = near {
        let start = text[..at].char_indices().rev().nth(30).map(|(i, _)| i).unwrap_or(0);
        let end = text[at..].char_indices().nth(30).map(|(i, _)| at + i).unwrap_or(text.len());
        if let Some(m) = re.find(&text[start..end]) {
            return Some(code(m.as_str()).to_string()).filter(|c| !c.is_empty());
        }
    }
    if let Some(m) = re.find(text) {
        return Some(code(m.as_str()).to_string()).filter(|c| !c.is_empty());
    }
    regex(r"\b(mva|totalt|kvittering|org\.?\s*nr|faktura)\b").is_match(text).then(|| "NOK".to_string())
}

pub fn find_order_number(text: &str) -> Option<String> {
    let re = regex(r"\b(?:ordre(?:nummer|nr\.?)?|order\s?(?:no\.?|number|#|id)|bestillingsnummer|kvitteringsnummer|kvittering for ordre|receipt\s?(?:no\.?|number|#)|faktura(?:nummer|nr\.?)?|invoice\s?(?:no\.?|number|#)|transaksjons?(?:id|nr\.?|nummer))\s*[:#.]?\s*([a-z0-9][a-z0-9-]{3,})");
    let found = re.captures_iter(text).filter_map(|c| c.get(1).map(|m| m.as_str().to_string())).find(|s| s.chars().any(|ch| ch.is_ascii_digit()));
    found
}

pub fn find_card_ending(text: &str) -> Option<String> {
    let re = regex(r"(?:\*{2,}|x{3,}|•{2,}|ending in|slutter på|kortnummer\s*:?|\b(?:visa|mastercard|amex|american express)\b[^\n\d]{0,20})\s*(\d{4})\b");
    re.captures(text).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

pub fn find_org_number(text: &str) -> Option<String> {
    let re = regex(r"\borg\.?\s*nr\.?\s*:?\s*(?:no\s*)?(\d{3})\s?(\d{3})\s?(\d{3})\b");
    re.captures(text).map(|c| format!("{} {} {}", &c[1], &c[2], &c[3]))
}

const MONTHS_NB: [&str; 12] = ["januar", "februar", "mars", "april", "mai", "juni", "juli", "august", "september", "oktober", "november", "desember"];
const MONTHS_EN: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

fn month(word: &str) -> Option<u32> {
    let w = word.to_lowercase();
    if w.chars().count() < 3 {
        return None;
    }
    MONTHS_NB.iter().chain(MONTHS_EN.iter()).position(|m| *m == w || m.starts_with(&w)).map(|i| (i % 12) as u32 + 1)
}

fn ymd(y: i32, m: u32, d: u32) -> Option<String> {
    chrono::NaiveDate::from_ymd_opt(y, m, d).map(|date| date.format("%Y-%m-%d").to_string())
}

pub fn find_date(text: &str) -> Option<String> {
    let dotted = regex(r"\b(\d{1,2})\.(\d{1,2})\.(\d{4})\b");
    let iso = regex(r"\b(\d{4})-(\d{2})-(\d{2})\b");
    let day_month = regex(r"\b(\d{1,2})\.?\s+([a-zæøå]{3,9})\.?\s+(\d{4})\b");
    let month_day = regex(r"\b([a-z]{3,9})\.?\s+(\d{1,2}),?\s+(\d{4})\b");
    let mut found: Vec<(usize, String)> = Vec::new();
    for c in dotted.captures_iter(text) {
        if let Some(d) = ymd(c[3].parse().ok()?, c[2].parse().ok()?, c[1].parse().ok()?) {
            found.push((c.get(0)?.start(), d));
        }
    }
    for c in iso.captures_iter(text) {
        if let Some(d) = ymd(c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?) {
            found.push((c.get(0)?.start(), d));
        }
    }
    for c in day_month.captures_iter(text) {
        if let Some(m) = month(&c[2]) {
            if let Some(d) = ymd(c[3].parse().ok()?, m, c[1].parse().ok()?) {
                found.push((c.get(0)?.start(), d));
            }
        }
    }
    for c in month_day.captures_iter(text) {
        if let Some(m) = month(&c[1]) {
            if let Some(d) = ymd(c[3].parse().ok()?, m, c[2].parse().ok()?) {
                found.push((c.get(0)?.start(), d));
            }
        }
    }
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().next().map(|(_, d)| d)
}

const STORES: [(&str, &str); 22] = [
    ("elkjøp", "Elkjøp"),
    ("elkjop", "Elkjøp"),
    ("komplett", "Komplett"),
    ("clas ohlson", "Clas Ohlson"),
    ("ikea", "IKEA"),
    ("apple", "Apple"),
    ("amazon", "Amazon"),
    ("zalando", "Zalando"),
    ("foodora", "foodora"),
    ("wolt", "Wolt"),
    ("uber", "Uber"),
    ("ruter", "Ruter"),
    ("vy", "Vy"),
    ("sas", "SAS"),
    ("norwegian", "Norwegian"),
    ("spotify", "Spotify"),
    ("netflix", "Netflix"),
    ("steam", "Steam"),
    ("vipps", "Vipps"),
    ("xxl", "XXL"),
    ("power", "POWER"),
    ("nordlys elektro", "Nordlys Elektro"),
];

pub fn find_vendor(candidate: &Value) -> String {
    let ai = scalar_text(candidate.get("ai").and_then(|a| a.get("vendor")));
    if !ai.trim().is_empty() {
        return ai.trim().to_string();
    }
    let sender = scalar_text(candidate.get("sender"));
    let email = scalar_text(candidate.get("sender_email")).to_lowercase();
    let haystack = format!("{} {}", sender.to_lowercase(), email);
    for (needle, name) in STORES {
        if regex(&format!(r"\b{}\b", regex::escape(needle))).is_match(&haystack) {
            return name.to_string();
        }
    }
    let cleaned = regex(r"(?i)\b(no-?reply|kvittering|receipts?|orders?|ordre|billing|faktura)\b").replace_all(&sender, "").trim().trim_matches(|c: char| c == '-' || c == '·' || c == '|').trim().to_string();
    if !cleaned.is_empty() {
        return cleaned;
    }
    email.split('@').nth(1).and_then(|d| d.split('.').next()).map(|d| {
        let mut chars = d.chars();
        chars.next().map(|f| f.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
    }).unwrap_or_default()
}

pub fn format_amount(value: f64) -> String {
    format!("{:.2}", value)
}

pub fn organise(candidates: &[Value]) -> Vec<Value> {
    candidates
        .iter()
        .filter(|c| c.get("confirmed").and_then(Value::as_bool) != Some(false))
        .map(|c| {
            let text = scalar_text(c.get("text"));
            let ai = c.get("ai").cloned().unwrap_or(Value::Null);
            let found_total = find_total(&text);
            let total = ai.get("total").and_then(Value::as_f64).or(found_total.map(|(v, _)| v));
            let currency = ai.get("currency").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.to_uppercase()).or_else(|| find_currency(&text, found_total.map(|(_, at)| at)));
            let date = ai.get("date").and_then(Value::as_str).filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()).map(String::from).or_else(|| find_date(&text)).or_else(|| Some(scalar_text(c.get("at")).chars().take(10).collect::<String>()).filter(|d| !d.is_empty()));
            let vendor = find_vendor(c);
            let score = c.get("score").and_then(Value::as_f64).unwrap_or(0.0);
            let confirmed = c.get("confirmed").and_then(Value::as_bool);
            let confidence = match confirmed {
                Some(true) => ai.get("confidence").and_then(Value::as_f64).unwrap_or(score.max(0.9)),
                _ => score,
            };
            let title = match (&total, &currency) {
                (Some(t), Some(cur)) => format!("{} · {} {}", vendor, format_amount(*t), cur),
                (Some(t), None) => format!("{} · {}", vendor, format_amount(*t)),
                _ => vendor.clone(),
            };
            let attachment = c.get("file").filter(|f| f.is_object()).map(|f| json!({ "id": f["id"], "index": f["index"], "name": f["name"], "content_type": f["content_type"], "size": f["size"], "path": f["path"], "method": f["method"] }));
            let mut record = Map::new();
            record.insert("id".into(), c["id"].clone());
            record.insert("kind".into(), json!("receipt"));
            record.insert("title".into(), json!(title));
            record.insert("vendor".into(), json!(vendor));
            record.insert("date".into(), json!(date));
            record.insert("total".into(), json!(total));
            record.insert("currency".into(), json!(currency));
            record.insert("vat".into(), json!(find_vat(&text)));
            record.insert("order_number".into(), json!(find_order_number(&format!("{}\n{}", scalar_text(c.get("subject")), text))));
            record.insert("card_ending".into(), json!(find_card_ending(&text)));
            record.insert("org_number".into(), json!(find_org_number(&text)));
            record.insert("provider".into(), c["provider"].clone());
            record.insert("connection".into(), c["connection"].clone());
            record.insert("conversation_id".into(), c["conversation_id"].clone());
            record.insert("conversation_name".into(), c["conversation_name"].clone());
            record.insert("message_id".into(), c["message_id"].clone());
            record.insert("subject".into(), c["subject"].clone());
            record.insert("sender".into(), c["sender"].clone());
            record.insert("attachment".into(), attachment.unwrap_or(Value::Null));
            record.insert("excerpt".into(), json!(capped(text.split_whitespace().collect::<Vec<_>>().join(" ").as_str(), EXCERPT)));
            record.insert("confidence".into(), json!((confidence * 100.0).round() / 100.0));
            record.insert("confirmed".into(), json!(confirmed));
            record.insert("score".into(), json!(score));
            record.insert("hits".into(), c["hits"].clone());
            Value::Object(record)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_conversation, test_message, test_pool, test_profile, test_provider};

    const RECEIPT_EML: &str = include_str!("../../tests/fixtures/mock/receipt.eml");

    fn body_of(eml: &str) -> String {
        eml.split_once("\n\n").map(|(_, b)| b.to_string()).unwrap_or_default()
    }

    fn material(subject: &str, sender: &str, email: &str, body: &str, files: Vec<(&str, &str, &str)>) -> Value {
        json!({
            "message_id": "m1",
            "conversation_id": "c1",
            "conversation_name": subject,
            "provider": "mock",
            "connection": "Mock Outlook",
            "subject": subject,
            "sender": sender,
            "sender_email": email,
            "at": "2026-10-05T15:41:07+00:00",
            "body": body,
            "files": files.iter().enumerate().map(|(i, (name, kind, text))| json!({ "id": format!("a{}", i), "index": i, "name": name, "content_type": kind, "size": text.len(), "path": format!("/cache/{}", name), "method": file_kind(name, kind), "chars": text.chars().count(), "text": text, "error": null })).collect::<Vec<_>>(),
        })
    }

    fn found(m: &Value) -> Vec<Value> {
        candidates(m, &default_patterns(), app_setting::get_default_f64(app_setting::RECEIPTS_THRESHOLD)).unwrap()
    }

    #[test]
    fn html_mail_becomes_readable_text() {
        let text = html_text(&body_of(RECEIPT_EML));
        assert!(text.contains("Takk for handelen, Milo!"), "{}", text);
        assert!(text.lines().any(|l| l.contains("Totalt") && l.contains("11 916,00")), "{}", text);
        assert!(!text.contains("<td"));
        let extracted = extract_bytes("kvittering.html", "text/html", body_of(RECEIPT_EML).as_bytes());
        assert_eq!(extracted.method, METHOD_HTML);
        assert!(extracted.error.is_none());
    }

    #[test]
    fn pdf_text_comes_out_of_a_receipt_pdf() {
        let bytes = crate::providers::mock::seed::pdf(&["Kvittering", "Elkjøp Norge AS", "Org.nr. 974 312 345 MVA", "Totalt 1 299,00 kr", "Mva 25 % 259,80"]);
        let extracted = extract_bytes("Kvittering-48213.pdf", "application/pdf", &bytes);
        assert_eq!(extracted.method, METHOD_PDF);
        assert!(extracted.error.is_none(), "{:?}", extracted.error);
        assert!(extracted.text.contains("Kvittering"), "{}", extracted.text);
        assert!(extracted.text.contains("Elkjøp"), "{}", extracted.text);
        assert!(extracted.text.contains("1 299,00"), "{}", extracted.text);
        let broken = extract_bytes("broken.pdf", "application/pdf", b"%PDF-1.4 nonsense");
        assert!(broken.error.is_some());
        assert_eq!(extract_bytes("tool.exe", "application/octet-stream", b"MZ").method, METHOD_SKIPPED);
    }

    #[test]
    fn the_matcher_finds_norwegian_and_english_receipts() {
        let nordlys = material("Kvittering for ordre 2026-48213", "Nordlys Elektro", "kvittering@nordlyselektro.no", &html_text(&body_of(RECEIPT_EML)), vec![]);
        let hit = found(&nordlys);
        assert_eq!(hit.len(), 1);
        assert!(hit[0]["score"].as_f64().unwrap() >= 0.9, "{}", hit[0]);
        let names: Vec<&str> = hit[0]["hits"].as_array().unwrap().iter().filter_map(|h| h["pattern"].as_str()).collect();
        for wanted in ["receipt_word", "total", "vat", "org_number", "order_number", "card_ending", "store_sender"] {
            assert!(names.contains(&wanted), "{} missing from {:?}", wanted, names);
        }

        let pdf = material("Din ordre hos Elkjøp", "Elkjøp", "ordre@elkjop.no", "Hei! Kvitteringen ligger vedlagt.", vec![("Kvittering-48213.pdf", "application/pdf", "Kvittering\nElkjøp Norge AS\nOrg.nr. 974 312 345 MVA\nTotalt 1 299,00 kr\nMva 25 % 259,80\nKort **** 4821")]);
        let hit = found(&pdf);
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0]["source"], "file");
        assert_eq!(hit[0]["id"], "m1#file0");

        let apple = material("Your receipt from Apple", "Apple", "no_reply@email.apple.com", "Receipt\nOrder ID: MQ7XK2LP9\nApple One Individual 1 month $19.95\nSubtotal $18.13\nTax $1.82\nTotal $19.95\nVisa •••• 1234", vec![]);
        assert_eq!(found(&apple).len(), 1);

        let paid = material("Invoice INV-2026-118 paid", "Fjord Hosting", "billing@fjordhosting.com", "Thanks, your invoice INV-2026-118 has been paid.\nAmount paid: EUR 49.00\nVAT 9.80", vec![]);
        assert_eq!(found(&paid).len(), 1);

        let ticket = material("Kvittering for kjøp av billett", "Ruter", "noreply@ruter.no", "Takk for kjøpet!\n30-dagersbillett sone 1\nSum 919,00 kr\nHerav mva 98,46 kr\nBetalt med Vipps", vec![]);
        assert_eq!(found(&ticket).len(), 1);
    }

    #[test]
    fn the_matcher_leaves_look_alikes_alone() {
        let unpaid = material("Ny faktura fra Telenor", "Telenor", "faktura@telenor.no", "Fakturanummer 99812\nÅ betale 649,00 kr\nForfallsdato 30.10.2026\nKID-nummer 1234567890", vec![]);
        assert!(found(&unpaid).is_empty(), "{:?}", found(&unpaid));
        let offer = material("Ukens tilbud: 30 % rabatt", "Kiwi", "nyhetsbrev@kiwi.no", "Ukens tilbud! Kjøp tre, betal for to. Totalt spar du 199 kr.\nMeld deg av nyhetsbrevet her.", vec![]);
        assert!(found(&offer).is_empty(), "{:?}", found(&offer));
        let application = material("Receipt of your application", "Acme Careers", "careers@acme.com", "We confirm receipt of your application for the role of designer. We will get back to you within two weeks.", vec![]);
        assert!(found(&application).is_empty(), "{:?}", found(&application));
        let shipped = material("Your order has shipped", "Amazon", "shipment-tracking@amazon.com", "Your package is on its way. Track your parcel with the link below.", vec![]);
        assert!(found(&shipped).is_empty(), "{:?}", found(&shipped));
        let sale = material("Price drop on items you viewed", "Zalando", "info@zalando.no", "Price drop! Sneakers now $79.00, was $120.00. Unsubscribe any time.", vec![]);
        assert!(found(&sale).is_empty(), "{:?}", found(&sale));
        let terms = material("Kvittering for ordre 77120", "Komplett", "ordre@komplett.no", "Takk for handelen! Totalt 2 499,00 kr inkl. mva.", vec![("Vilkår.pdf", "application/pdf", "Generelle vilkår for kjøp. Angrerett gjelder i fjorten dager.")]);
        let hit = found(&terms);
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0]["source"], "body");
    }

    #[test]
    fn patterns_are_editable_in_a_copy() {
        let custom = json!([{ "name": "kaffe", "pattern": "kaffekvittering", "weight": 0.9 }]);
        let patterns = patterns_from(&custom).unwrap();
        assert_eq!(patterns.len(), 1);
        let m = material("Kaffekvittering", "Kafe", "kafe@example.no", "", vec![]);
        assert_eq!(candidates(&m, &patterns, 0.5).unwrap().len(), 1);
        assert_eq!(patterns_from(&Value::Null).unwrap(), default_patterns());
        assert!(patterns_from(&json!([{ "pattern": 1 }])).is_err());
        assert!(hits(&[pattern("bad", "(", 1.0, &[])], &[(FIELD_BODY, "x")]).is_err());
    }

    #[test]
    fn amounts_read_in_both_formats() {
        assert_eq!(parse_amount("11 916,00"), Some(11916.0));
        assert_eq!(parse_amount("1,299.00"), Some(1299.0));
        assert_eq!(parse_amount("1.299,50"), Some(1299.5));
        assert_eq!(parse_amount("19.95"), Some(19.95));
        assert_eq!(parse_amount("2 490"), Some(2490.0));
        assert_eq!(parse_amount("1,299"), Some(1299.0));
        assert_eq!(parse_amount("149,-"), Some(149.0));
        assert_eq!(find_date("Ordre 2026-48213 · 5. oktober 2026"), Some("2026-10-05".into()));
        assert_eq!(find_date("Date: October 3, 2026"), Some("2026-10-03".into()));
        assert_eq!(find_date("Kjøpt 07.10.2026 kl 12:14"), Some("2026-10-07".into()));
    }

    #[test]
    fn the_organise_node_keeps_the_metadata_found_along_the_way() {
        let nordlys = material("Kvittering for ordre 2026-48213", "Nordlys Elektro", "kvittering@nordlyselektro.no", &html_text(&body_of(RECEIPT_EML)), vec![]);
        let mut found = found(&nordlys);
        let pdf = material("Din ordre hos Elkjøp", "Elkjøp", "ordre@elkjop.no", "Kvitteringen ligger vedlagt.", vec![("Kvittering-48213.pdf", "application/pdf", "Kvittering\nElkjøp Norge AS\nOrg.nr. 974 312 345 MVA\nDato 03.10.2026\nOrdrenummer 5521093\nTotalt 1 299,00 kr\nMva 25 % 259,80\nKort **** 4821")]);
        let mut second = candidates(&pdf, &default_patterns(), app_setting::get_default_f64(app_setting::RECEIPTS_THRESHOLD)).unwrap();
        second[0]["message_id"] = json!("m2");
        found.append(&mut second);
        let mut rejected = found[0].clone();
        rejected["id"] = json!("m3#body");
        rejected["confirmed"] = json!(false);
        found.push(rejected);
        found[1]["confirmed"] = json!(true);
        found[1]["ai"] = json!({ "id": found[1]["id"], "receipt": true, "confidence": 0.97 });
        let records = organise(&found);
        assert_eq!(records.len(), 2);
        let nordlys = &records[0];
        assert_eq!(nordlys["vendor"], "Nordlys Elektro");
        assert_eq!(nordlys["total"], 11916.0);
        assert_eq!(nordlys["currency"], "NOK");
        assert_eq!(nordlys["vat"], 2383.2);
        assert_eq!(nordlys["order_number"], "2026-48213");
        assert_eq!(nordlys["org_number"], "912 345 678");
        assert_eq!(nordlys["date"], "2026-10-05");
        assert_eq!(nordlys["provider"], "mock");
        assert_eq!(nordlys["message_id"], "m1");
        assert_eq!(nordlys["conversation_id"], "c1");
        assert!(nordlys["attachment"].is_null());
        assert!(nordlys["confirmed"].is_null());
        assert!(nordlys["excerpt"].as_str().unwrap().starts_with("Kvittering for ordre"));
        assert_eq!(nordlys["title"], "Nordlys Elektro · 11916.00 NOK");
        let elkjop = &records[1];
        assert_eq!(elkjop["vendor"], "Elkjøp");
        assert_eq!(elkjop["total"], 1299.0);
        assert_eq!(elkjop["vat"], 259.8);
        assert_eq!(elkjop["card_ending"], "4821");
        assert_eq!(elkjop["order_number"], "5521093");
        assert_eq!(elkjop["date"], "2026-10-03");
        assert_eq!(elkjop["attachment"]["name"], "Kvittering-48213.pdf");
        assert_eq!(elkjop["attachment"]["path"], "/cache/Kvittering-48213.pdf");
        assert_eq!(elkjop["confidence"], 0.97);
        assert_eq!(elkjop["confirmed"], true);
    }

    #[tokio::test]
    async fn without_a_local_model_candidates_pass_on_unconfirmed() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let input = crate::models::intelligence::PostIntelligenceConnectionInput {
            kind: KIND_OLLAMA.to_string(),
            name: "Ollama nowhere".to_string(),
            config: std::collections::BTreeMap::from([("host".to_string(), "http://127.0.0.1:9".to_string())]),
            is_default: Some(false),
        };
        let connection = IntelligenceConnection::post(&pool, &profile, input).await.unwrap();
        let m = material("Kvittering for ordre 2026-48213", "Nordlys Elektro", "kvittering@nordlyselektro.no", &html_text(&body_of(RECEIPT_EML)), vec![]);
        let list = found(&m);
        let (out, log) = confirm(&pool, &profile, list.clone(), 5, &json!({ "connection_id": connection.id }), None, None).await;
        assert_eq!(out.len(), list.len());
        assert!(out.iter().all(|c| c["confirmed"].is_null()));
        assert!(log.iter().any(|l| l.contains("No local model is available") && l.contains("unconfirmed")), "{:?}", log);
        assert_eq!(organise(&out).len(), 1);

        let ask: Ask = std::sync::Arc::new(|key: &str, extra: &Value| {
            assert_eq!(key, PROMPT_KEY);
            assert!(extra["prompt"].as_str().unwrap().contains("Nordlys Elektro"));
            Ok(json!({ "verdicts": [{ "id": "m1#body", "receipt": false, "confidence": 0.8 }] }))
        });
        let (judged, log) = confirm(&pool, &profile, list, 5, &Value::Null, None, Some(ask)).await;
        assert_eq!(judged[0]["confirmed"], false);
        assert!(log.iter().any(|l| l.contains("1 of 1")), "{:?}", log);
        assert!(organise(&judged).is_empty());
    }

    #[tokio::test]
    async fn material_reads_the_body_and_records_each_file() {
        let pool = test_pool().await;
        let profile = test_profile(&pool).await;
        let provider = test_provider(&pool, &profile, "mock").await;
        let conversation = test_conversation(&pool, "Elkjøp", "email").await;
        let message = test_message(&pool, &conversation, &provider, "<p>Kvitteringen ligger <b>vedlagt</b>.</p>").await;
        use base64::Engine as _;
        let pdf = base64::engine::general_purpose::STANDARD.encode(crate::providers::mock::seed::pdf(&["Kvittering", "Totalt 1 299,00 kr"]));
        let attachments = json!([
            { "id": "a1", "name": "Kvittering.pdf", "content_type": "application/pdf", "size": 900, "is_inline": false, "content_id": null, "url": format!("data:application/pdf;base64,{}", pdf) },
            { "id": "a2", "name": "logo.png", "content_type": "image/png", "size": 10, "is_inline": true, "content_id": "logo", "url": null },
            { "id": "a3", "name": "data.bin", "content_type": "application/octet-stream", "size": 10, "is_inline": false, "content_id": null, "url": null }
        ]);
        sqlx::query("UPDATE message SET attachments = ?1, content_type = 'html', subject = 'Din ordre' WHERE id = ?2").bind(attachments.to_string()).bind(&message).execute(&pool).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let m = super::material(&pool, dir.path(), dir.path(), &message).await.unwrap();
        assert_eq!(m["body"], "Kvitteringen ligger vedlagt.");
        let files = m["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["method"], "pdf");
        assert!(files[0]["text"].as_str().unwrap().contains("1 299,00"));
        assert!(files[0]["chars"].as_u64().unwrap() > 10);
        assert!(std::path::Path::new(files[0]["path"].as_str().unwrap()).is_file());
        assert_eq!(files[1]["method"], "skipped");
        assert!(files[1]["error"].is_string());
    }
}
