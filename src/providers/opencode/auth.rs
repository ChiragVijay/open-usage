use super::databases;
use crate::providers::{
    http::{read, token},
    local,
};
use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// An existing v2 credential table is authoritative, including after logout.
pub fn go_key(homes: &[PathBuf]) -> Result<Option<String>> {
    let mut migrated = false;
    for path in databases(homes)? {
        let connection = local::database(&path)
            .map_err(|_| anyhow::anyhow!("OpenCode credentials could not be read"))?;
        if !local::table(&connection, "credential")? {
            continue;
        }
        migrated = true;
        let key: Option<String> = connection.query_row("SELECT json_extract(value,'$.key') FROM credential WHERE integration_id='opencode-go' AND (active IS NULL OR active=1) AND json_valid(value) ORDER BY active DESC,time_updated DESC,id DESC LIMIT 1", [], |row| row.get(0)).optional()?.flatten();
        if let Some(key) = key.filter(|key| !key.trim().is_empty()) {
            return Ok(Some(key));
        }
    }
    if migrated {
        return Ok(None);
    }
    for home in homes {
        let path = home.join("auth.json");
        if !path.exists() {
            continue;
        }
        let auth = read(&path).context("OpenCode auth.json could not be read")?;
        if auth["opencode-go"]["type"] == "api"
            && let Some(key) = token(&auth, "/opencode-go/key")
        {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

pub(super) fn openai_credential(
    connection: &rusqlite::Connection,
    path: &Path,
) -> Result<Option<(Value, Option<i64>)>> {
    if local::table(connection, "credential")? {
        let row: Option<(String,i64)> = connection.query_row("SELECT value,time_created FROM credential WHERE integration_id='openai' AND (active IS NULL OR active=1) ORDER BY active DESC,time_updated DESC,id DESC LIMIT 1", [], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        return row
            .map(|(raw, stamp)| Ok((serde_json::from_str(&raw)?, Some(stamp))))
            .transpose();
    }
    Ok(
        read(&path.parent().unwrap_or(Path::new("")).join("auth.json"))
            .and_then(|auth| auth.get("openai").cloned())
            .map(|value| (value, None)),
    )
}
pub fn codex_auth(homes: &[PathBuf]) -> Result<Option<Value>> {
    for path in databases(homes)? {
        if let Some((auth, _)) = openai_credential(&local::database(&path)?, &path)?
            && auth["type"] == "oauth"
            && token(&auth, "/access").is_some()
        {
            return Ok(Some(auth));
        }
    }
    Ok(None)
}
