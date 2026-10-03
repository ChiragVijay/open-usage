use super::auth::openai_credential;
use crate::model::{Provider, Tokens, UsageRecord};
use crate::providers::{
    history::{hash, uint},
    http::token,
    local,
};
use anyhow::Result;
use chrono::DateTime;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

pub fn history(path: &Path) -> Result<Vec<UsageRecord>> {
    history_for(path, false)
}
pub fn codex_history(path: &Path) -> Result<Vec<UsageRecord>> {
    history_for(path, true)
}
fn history_for(path: &Path, codex: bool) -> Result<Vec<UsageRecord>> {
    let connection = local::database(path)?;
    let oauth = if codex {
        openai_credential(&connection, path)?
    } else {
        None
    };
    if codex
        && oauth.as_ref().is_none_or(|(auth, _)| {
            auth["type"] != "oauth"
                || (token(auth, "/access").is_none() && token(auth, "/refresh").is_none())
        })
    {
        return Ok(vec![]);
    }
    let mut records = BTreeMap::new();
    let mut projects = BTreeMap::<String, String>::new();
    if local::table(&connection, "session")? {
        let has_directory: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('session') WHERE name='directory')",
            [],
            |row| row.get(0),
        )?;
        if has_directory {
            let mut query = connection.prepare("SELECT id,directory FROM session")?;
            for row in query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            })? {
                let (id, directory) = row?;
                projects.insert(id, directory);
            }
        }
    }
    // Read v2 first: migrated rows retain their IDs, so v1 duplicates lose.
    for (table, role) in [
        (
            "session_message",
            "(type='assistant' OR (type='compaction' AND json_extract(data,'$.status')='completed'))",
        ),
        ("message", "json_extract(data,'$.role')='assistant'"),
    ] {
        if !local::table(&connection, table)? {
            continue;
        }
        let has_session: bool = connection.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name='session_id')"
            ),
            [],
            |row| row.get(0),
        )?;
        let session = if has_session {
            "session_id"
        } else {
            "json_extract(data,'$.sessionID')"
        };
        // Select accounting fields only; prompt and response content stay in the agent's DB.
        let filter = if codex {
            "AND COALESCE(json_extract(data,'$.model.providerID'),json_extract(data,'$.providerID'))='openai' AND json_extract(data,'$.cost')=0 AND (json_type(data,'$.time.completed') IN ('integer','real') OR json_type(data,'$.finish')='text' OR json_extract(data,'$.status')='completed')"
        } else {
            "AND COALESCE(json_extract(data,'$.model.providerID'),json_extract(data,'$.providerID')) IN ('opencode','opencode-go') AND json_type(data,'$.cost') IN ('integer','real')"
        };
        let sql = format!(
            "SELECT id,COALESCE(json_extract(data,'$.time.completed'),time_created),json_object('session', {session}, 'model',COALESCE(json_extract(data,'$.model.id'),json_extract(data,'$.modelID')),'provider',COALESCE(json_extract(data,'$.model.providerID'),json_extract(data,'$.providerID')),'tokens',json_extract(data,'$.tokens'),'cost',json_extract(data,'$.cost')) FROM {table} WHERE json_valid(data) AND {role} {filter}"
        );
        let mut query = connection.prepare(&sql)?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (id, ms, raw) = row?;
            if codex
                && table == "session_message"
                && oauth
                    .as_ref()
                    .and_then(|(_, since)| *since)
                    .is_some_and(|since| ms < since)
            {
                continue;
            }
            let data: Value = serde_json::from_str(&raw)?;
            let Some(timestamp) = DateTime::from_timestamp_millis(ms) else {
                continue;
            };
            let session = data["session"].as_str().unwrap_or("").to_owned();
            let usage = &data["tokens"];
            let tokens = Tokens {
                input: uint(&usage["input"]),
                output: uint(&usage["output"]).saturating_add(uint(&usage["reasoning"])),
                cache_read: uint(&usage["cache"]["read"]),
                cache_write: uint(&usage["cache"]["write"]),
            };
            let cost = if codex {
                None
            } else {
                data["cost"]
                    .as_f64()
                    .filter(|cost| cost.is_finite() && *cost >= 0.)
            };
            if tokens.total() == 0 && cost.is_none_or(|cost| cost == 0.) {
                continue;
            }
            let id = format!("opencode:{}", hash(&id));
            records.entry(id.clone()).or_insert(UsageRecord {
                id,
                provider: if codex {
                    Provider::Codex
                } else {
                    Provider::OpenCode
                },
                timestamp,
                model: data["model"].as_str().unwrap_or("Unknown model").into(),
                project: projects.get(&session).cloned().unwrap_or_default(),
                session,
                tokens,
                reported_cost: cost,
                source: path.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(records.into_values().collect())
}
