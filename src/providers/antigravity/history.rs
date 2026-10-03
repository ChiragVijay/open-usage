use crate::model::{Provider, Tokens, UsageRecord};
use crate::providers::{history::hash, local, proto};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

pub fn databases(homes: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut paths = vec![];
    for home in homes {
        paths.extend(local::files(&home.join("conversations"), "", ".db")?);
    }
    paths = paths
        .into_iter()
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn generation(blob: &[u8], step: Option<&[u8]>) -> Option<(DateTime<Utc>, String, Tokens)> {
    let wrapped = proto::bytes(blob, 1)?;
    let id = proto::text(wrapped, 19).map(|id| id.trim_end_matches("-tiered").to_owned());
    let label = proto::text(wrapped, 21);
    let usage = proto::bytes(wrapped, 4)?;
    let system = proto::integer(usage, 1).unwrap_or(0);
    let input = proto::integer(usage, 2).unwrap_or(0);
    let output = proto::integer(usage, 3).unwrap_or(0);
    let cache_read = proto::integer(usage, 5).unwrap_or(0);
    if id.is_none() && label.is_none() && input == 0 && output == 0 && cache_read == 0 {
        return None;
    }
    let tokens = Tokens {
        input: system.checked_add(input)?,
        output,
        cache_read,
        cache_write: 0,
    };
    if tokens.total() == 0 {
        return None;
    }
    let stamp = proto::bytes(wrapped, 9)
        .and_then(|timing| proto::bytes(timing, 4))
        .and_then(|stamp| proto::integer(stamp, 1))
        .or_else(|| {
            step.and_then(|step| proto::bytes(step, 1))
                .and_then(|stamp| proto::integer(stamp, 1))
        })?;
    let timestamp = DateTime::from_timestamp(i64::try_from(stamp).ok()?, 0)?;
    let model = if id.as_deref().is_some_and(|id| id.ends_with("-default")) {
        label.or(id)
    } else {
        id.or(label)
    }
    .unwrap_or_else(|| "Unknown Antigravity model".into());
    Some((timestamp, model_name(&model), tokens))
}
fn model_name(model: &str) -> String {
    let key = model.trim().to_ascii_lowercase();
    // Effort and subagent labels describe the same served model, not a new rate.
    if key.starts_with("gemini ") {
        let words = key
            .split_whitespace()
            .take_while(|word| !word.starts_with('('))
            .collect::<Vec<_>>();
        if words.len() >= 3 && words[1].chars().all(|c| c.is_ascii_digit() || c == '.') {
            return words.join("-");
        }
    }
    if key.starts_with("gemini-") {
        return key
            .trim_end_matches("-high")
            .trim_end_matches("-low")
            .to_owned();
    }
    model.to_owned()
}

pub fn history(path: &Path) -> Result<Vec<UsageRecord>> {
    Ok(history_with_warnings(path)?.0)
}
pub fn history_with_warnings(path: &Path) -> Result<(Vec<UsageRecord>, usize)> {
    let connection = local::database(path)?;
    if !local::table(&connection, "gen_metadata")? {
        return Ok((vec![], 0));
    }
    let steps: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('steps') WHERE name='metadata')",
        [],
        |row| row.get(0),
    )?;
    let step_sql = if steps {
        "(SELECT CASE WHEN length(metadata)<=1048576 THEN metadata END FROM steps WHERE idx=g.idx)"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT idx,CASE WHEN length(data)<=1048576 THEN data END,{step_sql} FROM gen_metadata g ORDER BY idx"
    );
    let mut query = connection.prepare(&sql)?;
    let mut records = vec![];
    let mut oversized = 0;
    for row in query.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<Vec<u8>>>(1)?,
            row.get::<_, Option<Vec<u8>>>(2)?,
        ))
    })? {
        let (index, blob, step) = row?;
        let Some(blob) = blob else {
            oversized += 1;
            continue;
        };
        if let Some((timestamp, model, tokens)) = generation(&blob, step.as_deref()) {
            let session = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            records.push(UsageRecord {
                id: format!("antigravity:{}", hash(&format!("{session}:{index}"))),
                provider: Provider::Antigravity,
                timestamp,
                model,
                session,
                project: String::new(),
                tokens,
                reported_cost: None,
                source: path.to_string_lossy().into_owned(),
            });
        }
    }
    Ok((records, oversized))
}
