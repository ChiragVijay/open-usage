use crate::model::{Provider, Tokens, UsageRecord};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub fn date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.with_timezone(&Utc))
        .or_else(|| {
            value.parse::<i64>().ok().and_then(|v| {
                DateTime::from_timestamp_millis(if v < 10_000_000_000 {
                    v.saturating_mul(1000)
                } else {
                    v
                })
            })
        })
}
pub fn uint(value: &Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|v| v.replace(',', "").parse().ok()))
        .unwrap_or(0)
}
fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or("")
}
fn project(value: &str) -> String {
    value
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .into()
}
fn codex_tokens(value: &Value) -> Tokens {
    let read = uint(&value["cached_input_tokens"]);
    Tokens {
        input: uint(&value["input_tokens"]).saturating_sub(read),
        output: uint(&value["output_tokens"]),
        cache_read: read,
        cache_write: 0,
    }
}
fn delta(value: &Tokens, old: &Tokens) -> Tokens {
    Tokens {
        input: value.input.saturating_sub(old.input),
        output: value.output.saturating_sub(old.output),
        cache_read: value.cache_read.saturating_sub(old.cache_read),
        cache_write: value.cache_write.saturating_sub(old.cache_write),
    }
}
pub struct Parsed {
    pub records: Vec<UsageRecord>,
    pub skipped: usize,
}

pub fn read_jsonl(path: &Path, provider: Provider) -> Result<Parsed> {
    let mut parsed = Parsed {
        records: vec![],
        skipped: 0,
    };
    let mut model = "unknown".to_owned();
    let mut session = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut cwd = String::new();
    let mut previous = Tokens::default();
    let mut previous_total: Option<u64> = None;
    let mut claude: BTreeMap<String, UsageRecord> = BTreeMap::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                parsed.skipped += 1;
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                parsed.skipped += 1;
                continue;
            }
        };
        if provider == Provider::Codex {
            let payload = &value["payload"];
            if text(&value, "type") == "session_meta" {
                if !text(payload, "id").is_empty() {
                    session = text(payload, "id").into();
                }
                cwd = project(text(payload, "cwd"));
                if !text(payload, "model").is_empty() {
                    model = text(payload, "model").into();
                }
            }
            if text(&value, "type") == "turn_context" && !text(payload, "model").is_empty() {
                model = text(payload, "model").into();
            }
            if text(payload, "type") != "token_count" {
                continue;
            }
            let Some(timestamp) = date(text(&value, "timestamp")) else {
                parsed.skipped += 1;
                continue;
            };
            let info = &payload["info"];
            if info.is_null() {
                continue;
            }
            let total = &info["total_token_usage"];
            let last = &info["last_token_usage"];
            let tokens = if total.is_object() {
                let current = codex_tokens(total);
                let total_count = uint(&total["total_tokens"]).max(current.total());
                if previous_total == Some(total_count) {
                    continue;
                }
                let tokens = if previous_total.is_some_and(|old| total_count < old) {
                    if last.is_object() {
                        codex_tokens(last)
                    } else {
                        current.clone()
                    }
                } else {
                    delta(&current, &previous)
                };
                previous = current;
                previous_total = Some(total_count);
                tokens
            } else if last.is_object() {
                codex_tokens(last)
            } else {
                continue;
            };
            if tokens.total() == 0 {
                continue;
            }
            let id = hash(&format!(
                "codex:{session}:{timestamp}:{previous_total:?}:{tokens:?}"
            ));
            parsed.records.push(UsageRecord {
                id,
                provider,
                timestamp,
                model: model.clone(),
                session: session.clone(),
                project: cwd.clone(),
                tokens,
                reported_cost: None,
                source: path.to_string_lossy().into_owned(),
            });
        } else if provider == Provider::Claude {
            if text(&value, "type") != "assistant" {
                continue;
            }
            let message = &value["message"];
            let usage = &message["usage"];
            if !usage.is_object() {
                continue;
            }
            let Some(timestamp) = date(text(&value, "timestamp")) else {
                parsed.skipped += 1;
                continue;
            };
            let message_id = text(message, "id");
            let session_id = if text(&value, "sessionId").is_empty() {
                session.as_str()
            } else {
                text(&value, "sessionId")
            };
            let request = text(&value, "requestId");
            let identity = if message_id.is_empty() {
                text(&value, "uuid")
            } else {
                message_id
            };
            let id = if identity.is_empty() {
                hash(&format!("claude:{session_id}:{timestamp}:{usage}"))
            } else {
                hash(&format!("claude:{identity}:{request}"))
            };
            let tokens = Tokens {
                input: uint(&usage["input_tokens"]),
                output: uint(&usage["output_tokens"]),
                cache_read: uint(&usage["cache_read_input_tokens"]),
                cache_write: uint(&usage["cache_creation_input_tokens"]),
            };
            let reported_cost = value["costUSD"]
                .as_f64()
                .filter(|cost| cost.is_finite() && *cost >= 0.0);
            if tokens.total() == 0 && reported_cost.is_none() {
                continue;
            }
            // Streaming content blocks repeat the same message. Keep the fullest usage.
            if claude
                .get(&id)
                .is_some_and(|old| old.tokens.total() > tokens.total())
            {
                continue;
            }
            claude.insert(
                id.clone(),
                UsageRecord {
                    id,
                    provider,
                    timestamp,
                    model: if text(message, "model").is_empty() {
                        "unknown"
                    } else {
                        text(message, "model")
                    }
                    .into(),
                    session: session_id.into(),
                    project: project(text(&value, "cwd")),
                    tokens,
                    reported_cost,
                    source: path.to_string_lossy().into_owned(),
                },
            );
        }
    }
    parsed.records.extend(claude.into_values());
    Ok(parsed)
}

pub fn read_cursor_csv(path: &Path) -> Result<Vec<UsageRecord>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers: Vec<String> = reader
        .headers()?
        .iter()
        .map(|h| {
            h.trim()
                .trim_start_matches('\u{feff}')
                .to_lowercase()
                .replace([' ', '_', '-'], "")
        })
        .collect();
    let index = |keys: &[&str]| headers.iter().position(|h| keys.contains(&h.as_str()));
    let timestamp =
        index(&["date", "timestamp"]).context("CSV needs a Date or Timestamp column")?;
    let model = index(&["model"]).context("CSV needs a Model column")?;
    let input = index(&[
        "inputtokens",
        "inputtokens(w/ocachewrite)",
        "uncachedinputtokens",
    ]);
    let output = index(&["outputtokens"]);
    let read = index(&["cachereadtokens", "cache(d)readtokens"]);
    let write = index(&[
        "cachewritetokens",
        "cachecreationtokens",
        "inputtokens(w/cachewrite)",
    ]);
    let cost = index(&["cost", "cost($)", "totalcost", "costusd"]);
    if input.is_none() && output.is_none() && cost.is_none() {
        bail!("CSV needs token or cost columns from Cursor's usage export");
    }
    let mut records = vec![];
    let mut occurrences = HashMap::new();
    for row in reader.records() {
        let row = row?;
        let field = |index: Option<usize>| index.and_then(|i| row.get(i)).unwrap_or("").trim();
        let timestamp =
            date(row.get(timestamp).unwrap_or("")).context("CSV contains an invalid timestamp")?;
        let number = |index| -> Result<u64> {
            let value = field(index);
            if value.is_empty() {
                Ok(0)
            } else {
                Ok(value
                    .replace(',', "")
                    .parse::<u64>()
                    .context("CSV token counts must be nonnegative integers")?)
            }
        };
        let tokens = Tokens {
            input: number(input)?,
            output: number(output)?,
            cache_read: number(read)?,
            cache_write: number(write)?,
        };
        let reported_cost =
            if field(cost).is_empty() || field(cost).eq_ignore_ascii_case("included") {
                None
            } else {
                let cost: f64 = field(cost)
                    .trim_start_matches('$')
                    .replace(',', "")
                    .parse()
                    .context("CSV contains an invalid cost")?;
                if !cost.is_finite() || cost < 0.0 {
                    bail!("CSV costs must be nonnegative");
                }
                Some(cost)
            };
        let model = row.get(model).unwrap_or("unknown").to_owned();
        let key = hash(&format!(
            "cursor:{timestamp}:{model}:{tokens:?}:{reported_cost:?}"
        ));
        let occurrence = occurrences.entry(key.clone()).or_insert(0);
        let id = format!("{key}:{occurrence}");
        *occurrence += 1;
        records.push(UsageRecord {
            id,
            provider: Provider::Cursor,
            timestamp,
            model,
            session: String::new(),
            project: "Cursor account".into(),
            tokens,
            reported_cost,
            source: path.to_string_lossy().into_owned(),
        });
    }
    Ok(records)
}
