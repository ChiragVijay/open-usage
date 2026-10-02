use super::history::{date, hash, uint};
use crate::model::{LimitWindow, Provider, Tokens, UsageRecord};
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use reqwest::blocking::{Client, RequestBuilder};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct LiveData {
    pub limits: Vec<LimitWindow>,
    pub plan: Option<String>,
    pub records: Vec<UsageRecord>,
}
fn read(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
}
fn token(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
}

#[cfg(target_os = "macos")]
fn keychain(service: &str, account: Option<&str>) -> Option<String> {
    use std::{
        process::{Command, Stdio},
        thread,
    };
    let mut command = Command::new("/usr/bin/security");
    command.args(["find-generic-password", "-s", service, "-w"]);
    if let Some(account) = account {
        command.args(["-a", account]);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return child
                    .wait_with_output()
                    .ok()
                    .and_then(|out| String::from_utf8(out.stdout).ok())
                    .map(|s| s.trim().into());
            }
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn keychain(_service: &str, _account: Option<&str>) -> Option<String> {
    None
}

fn response(request: RequestBuilder, provider: Provider) -> Result<Value> {
    let response = request.send().map_err(|_| {
        anyhow!(
            "{} could not be reached. Cached values are retained.",
            provider.name()
        )
    })?;
    if matches!(response.status().as_u16(), 401 | 403) {
        bail!("Sign in to {} again, then refresh", provider.name());
    }
    if !response.status().is_success() {
        bail!(
            "{} account check returned HTTP {}. Refresh to retry.",
            provider.name(),
            response.status().as_u16()
        );
    }
    response
        .json()
        .map_err(|_| anyhow!("{} returned an unreadable usage response", provider.name()))
}
fn window(label: &str, used: &Value, reset: &Value, duration: Option<u64>) -> Option<LimitWindow> {
    let used = used.as_f64().filter(|v| v.is_finite())?;
    let resets_at = reset.as_str().and_then(date).or_else(|| {
        reset.as_i64().and_then(|at| {
            if at > 10_000_000_000 {
                DateTime::from_timestamp_millis(at)
            } else {
                DateTime::from_timestamp(at, 0)
            }
        })
    });
    Some(LimitWindow {
        label: label.into(),
        used_percent: used.clamp(0.0, 100.0),
        resets_at,
        duration_minutes: duration,
    })
}

pub fn codex_limits(body: &Value) -> Vec<LimitWindow> {
    let bucket = body
        .pointer("/rateLimitsByLimitId/codex")
        .or_else(|| body.get("rateLimits"))
        .unwrap_or(&body["rate_limit"]);
    let mut limits = vec![];
    for (snake, camel, fallback) in [
        ("primary_window", "primary", 300),
        ("secondary_window", "secondary", 10080),
    ] {
        let data = bucket
            .get(snake)
            .or_else(|| bucket.get(camel))
            .unwrap_or(&Value::Null);
        let mins = data["windowDurationMins"]
            .as_u64()
            .or_else(|| data["limit_window_seconds"].as_u64().map(|v| v / 60))
            .unwrap_or(fallback);
        let label = if mins >= 43200 {
            "Monthly"
        } else if mins >= 10080 {
            "Weekly"
        } else {
            "Session"
        };
        if let Some(limit) = window(
            label,
            data.get("used_percent").unwrap_or(&data["usedPercent"]),
            data.get("reset_at").unwrap_or(&data["resetsAt"]),
            Some(mins),
        ) {
            limits.push(limit);
        }
    }
    limits
}
pub fn claude_limits(body: &Value) -> Vec<LimitWindow> {
    let mut limits = vec![];
    for (key, label, mins) in [
        ("five_hour", "5-hour session", 300),
        ("seven_day", "Weekly", 10080),
        ("seven_day_sonnet", "Weekly · Sonnet", 10080),
        ("seven_day_opus", "Weekly · Opus", 10080),
    ] {
        if let Some(limit) = window(
            label,
            &body[key]["utilization"],
            &body[key]["resets_at"],
            Some(mins),
        ) {
            limits.push(limit);
        }
    }
    if let Some(scoped) = body["model_scoped"].as_array() {
        for data in scoped {
            let label = format!(
                "Weekly · {}",
                data["display_name"].as_str().unwrap_or("Model")
            );
            if let Some(limit) = window(
                &label,
                &data["utilization"],
                &data["resets_at"],
                Some(10080),
            ) {
                limits.push(limit);
            }
        }
    }
    limits
}
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

pub fn fetch(provider: Provider, homes: &[PathBuf], history: bool) -> Result<LiveData> {
    let client = Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("OpenUsage/", env!("CARGO_PKG_VERSION")))
        .build()?;
    match provider {
        Provider::Codex => {
            let auth = homes
                .iter()
                .find_map(|home| read(&home.join("auth.json")))
                .context("Sign in with `codex login` to read ChatGPT limits")?;
            let access = token(&auth, "/tokens/access_token")
                .context("An API-key login does not include ChatGPT subscription limits")?;
            let mut request = client
                .get("https://chatgpt.com/backend-api/wham/usage")
                .bearer_auth(&access)
                .header("Accept", "application/json");
            if let Some(account) = token(&auth, "/tokens/account_id") {
                request = request.header("ChatGPT-Account-Id", account);
            }
            let body = response(request, provider)?;
            let limits = codex_limits(&body);
            if limits.is_empty() {
                bail!("This ChatGPT account did not report subscription limits");
            }
            Ok(LiveData {
                limits,
                plan: token(&body, "/plan_type"),
                records: vec![],
            })
        }
        Provider::Claude => {
            let auth = homes
                .iter()
                .find_map(|home| read(&home.join(".credentials.json")))
                .or_else(|| {
                    keychain("Claude Code-credentials", None)
                        .and_then(|data| serde_json::from_str(&data).ok())
                });
            let access = std::env::var("CLAUDE_CODE_OAUTH_TOKEN")
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    auth.as_ref()
                        .and_then(|auth| token(auth, "/claudeAiOauth/accessToken"))
                })
                .context("Sign in to Claude Code to read subscription limits")?;
            let body = response(
                client
                    .get("https://api.anthropic.com/api/oauth/usage")
                    .bearer_auth(access)
                    .header("anthropic-beta", "oauth-2025-04-20")
                    .header("Accept", "application/json"),
                provider,
            )?;
            let limits = claude_limits(&body);
            if limits.is_empty() {
                bail!("This Claude account did not report subscription limits");
            }
            let plan = auth
                .as_ref()
                .and_then(|auth| token(auth, "/claudeAiOauth/subscriptionType"));
            Ok(LiveData {
                limits,
                plan,
                records: vec![],
            })
        }
        Provider::Cursor => {
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
                    .post(
                        "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage",
                    )
                    .bearer_auth(&access)
                    .header("connect-protocol-version", "1")
                    .header("x-cursor-client-type", "cli")
                    .json(&json!({})),
                provider,
            )?;
            let records = if history {
                cursor_history(&client, &access)?
            } else {
                Vec::new()
            };
            Ok(LiveData {
                limits: cursor_limits(&body),
                plan: token(&body, "/membershipType").or_else(|| token(&body, "/plan")),
                records,
            })
        }
    }
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
