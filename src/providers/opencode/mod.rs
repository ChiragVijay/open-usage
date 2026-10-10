//! Go and Zen accounting; verified ChatGPT OAuth calls are attributed to Codex.
mod auth;
mod history;
mod quota;
use crate::providers::{live::LiveData, local};
use anyhow::{Result, bail};
pub use auth::{codex_auth, go_key};
use chrono::Utc;
pub use history::{codex_history, history};
pub use quota::limits;
use reqwest::blocking::Client;
use serde_json::Value;
use std::path::PathBuf;

pub(super) fn fingerprint(homes: &[PathBuf]) -> Result<Option<String>> {
    Ok(go_key(homes)?.map(|key| crate::providers::history::hash(&key)))
}

pub fn databases(homes: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut paths = vec![];
    for home in homes {
        paths.extend(local::files(home, "opencode", ".db")?);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn fetch(client: &Client, homes: &[PathBuf]) -> Result<LiveData> {
    let Some(key) = go_key(homes)? else {
        return Ok(LiveData {
            limits: vec![],
            plan: None,
            records: vec![],
        });
    };
    let result = client
        .get("https://opencode.ai/zen/go/v1/usage")
        .bearer_auth(key)
        .header("Accept", "application/json")
        .send()
        .map_err(|_| {
            anyhow::anyhow!("OpenCode could not be reached. Cached values are retained.")
        })?;
    let status = result.status();
    let body: Value = crate::input::json(result, crate::input::RESPONSE_BYTES)
        .map_err(|_| anyhow::anyhow!("OpenCode returned an unreadable usage response"))?;
    if status.as_u16() == 403
        && body.pointer("/error/type").and_then(Value::as_str) == Some("EntitlementError")
    {
        return Ok(LiveData {
            limits: vec![],
            plan: None,
            records: vec![],
        });
    }
    if matches!(status.as_u16(), 401 | 403) {
        bail!("Sign in to OpenCode Go again, then refresh");
    }
    if !status.is_success() {
        bail!(
            "OpenCode account check returned HTTP {}. Refresh to retry.",
            status.as_u16()
        );
    }
    Ok(LiveData {
        limits: limits(&body, Utc::now())?,
        plan: Some("Go".into()),
        records: vec![],
    })
}
