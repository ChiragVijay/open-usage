use super::{
    history::{date, hash, uint},
    http::{keychain, read, response, token, window},
    live::LiveData,
};
use crate::model::{LimitWindow, Provider, Tokens, UsageRecord};
use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

pub fn cursor_limits(body: &Value) -> Vec<LimitWindow> {
    [
        ("totalPercentUsed", "Monthly"),
        ("autoPercentUsed", "Auto"),
        ("apiPercentUsed", "API usage"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        window(
            label,
            &body["planUsage"][key],
            &body["billingCycleEnd"],
            None,
        )
    })
    .collect()
}

pub fn fetch(client: &Client, homes: &[PathBuf], history: bool) -> Result<LiveData> {
    let provider = Provider::Cursor;

    let access = std::env::var("CURSOR_AUTH_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| {
            homes.iter().find_map(|home| {
                read(&home.join("auth.json")).and_then(|auth| token(&auth, "/accessToken"))
            })
        })
        .or_else(|| cursor_desktop_token(homes))
        .or_else(|| keychain("cursor-access-token", Some("cursor-user")))
        .context("Sign in to Cursor CLI, or import a usage CSV")?;
    let body = response(
        client
            .post("https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage")
            .bearer_auth(&access)
            .header("connect-protocol-version", "1")
            .header("x-cursor-client-type", "cli")
            .json(&json!({})),
        provider,
    )?;
    let records = if history {
        cursor_history(client, &access)?
    } else {
        Vec::new()
    };
    Ok(LiveData {
        limits: cursor_limits(&body),
        plan: token(&body, "/membershipType").or_else(|| token(&body, "/plan")),
        records,
    })
}
fn cursor_desktop_token(homes: &[PathBuf]) -> Option<String> {
    for home in homes {
        let path = home.join("User/globalStorage/state.vscdb");
        if !path.exists() {
            continue;
        }
        if let Ok(connection) =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            && let Ok(value) = connection.query_row(
                "SELECT value FROM ItemTable WHERE key='cursorAuth/accessToken'",
                [],
                |row| row.get::<_, String>(0),
            )
        {
            return Some(value);
        }
    }
    None
}
fn cursor_history(client: &Client, access: &str) -> Result<Vec<UsageRecord>> {
    let claims: Value = access
        .split('.')
        .nth(1)
        .and_then(|data| URL_SAFE_NO_PAD.decode(data).ok())
        .and_then(|data| serde_json::from_slice(&data).ok())
        .context("Cursor login is unreadable. Sign in to Cursor again.")?;
    let subject = claims["sub"]
        .as_str()
        .context("Cursor login did not identify an account")?;
    let user = subject.rsplit('|').next().unwrap_or(subject);
    let cookie = format!(
        "WorkosCursorSessionToken={}",
        urlencoding::encode(&format!("{user}::{access}"))
    );
    let now = Utc::now();
    let since = now - chrono::Duration::days(90);
    let mut events = vec![];
    let mut completed = false;
    let mut expected = None;
    let deadline = Instant::now() + Duration::from_secs(60);
    for page in 1..=1000 {
        if Instant::now() >= deadline {
            bail!("Cursor history timed out. Previous results are retained.");
        }
        let body = response(client.post("https://cursor.com/api/dashboard/get-filtered-usage-events").header("Origin","https://cursor.com").header("Cookie",&cookie).json(&json!({"page":page,"pageSize":1000,"startDate":since.timestamp_millis().to_string(),"endDate":now.timestamp_millis().to_string()})),Provider::Cursor)?;
        let batch = body["usageEventsDisplay"]
            .as_array()
            .context("Cursor returned an unsupported history format")?;
        if let Some(total) = body["totalUsageEventsCount"].as_u64() {
            if expected.is_some_and(|old| old != total) {
                bail!("Cursor history changed during refresh. Refresh to retry.");
            }
            expected = Some(total);
        }
        events.extend(batch.iter().cloned());
        if batch.len() < 1000 {
            completed = true;
            break;
        }
    }
    if !completed || expected.is_some_and(|total| total != events.len() as u64) {
        bail!("Cursor returned incomplete history. Previous results are retained.");
    }
    parse_cursor_events(&events, &hash(subject))
}
pub fn parse_cursor_events(events: &[Value], account: &str) -> Result<Vec<UsageRecord>> {
    let mut records = vec![];
    let mut occurrences = HashMap::new();
    for event in events {
        let usage = &event["tokenUsage"];
        if !usage.is_object() {
            continue;
        }
        let stamp = event["timestamp"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| event["timestamp"].to_string());
        let timestamp = date(&stamp).context("Cursor returned an invalid timestamp")?;
        let model = event["model"]
            .as_str()
            .context("Cursor returned an unnamed model")?
            .to_owned();
        let tokens = Tokens {
            input: uint(&usage["inputTokens"]),
            output: uint(&usage["outputTokens"]),
            cache_read: uint(&usage["cacheReadTokens"]),
            cache_write: uint(&usage["cacheWriteTokens"]),
        };
        let reported_cost = usage["totalCents"]
            .as_f64()
            .filter(|cost| cost.is_finite() && *cost >= 0.0)
            .map(|cost| cost / 100.0);
        let key = hash(&format!(
            "{account}:{timestamp}:{model}:{tokens:?}:{reported_cost:?}"
        ));
        let occurrence = occurrences.entry(key.clone()).or_insert(0);
        let id = format!("{key}:{occurrence}");
        *occurrence += 1;
        records.push(UsageRecord {
            id,
            provider: Provider::Cursor,
            timestamp,
            model,
            session: event["conversationId"].as_str().unwrap_or("").into(),
            project: "Cursor account".into(),
            tokens,
            reported_cost,
            source: "Cursor account API".into(),
        });
    }
    Ok(records)
}
