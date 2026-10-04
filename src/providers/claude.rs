use super::{
    http::{keychain, read, response, token, window},
    live::LiveData,
};
use crate::model::{LimitWindow, Provider};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::Value;
use std::path::PathBuf;

pub fn claude_limits(body: &Value) -> Vec<LimitWindow> {
    let mut limits = vec![];
    let weekly = |name: &str| format!("Weekly{}{name}", crate::analytics::separator());
    for (key, label, mins) in [
        ("five_hour", "5-hour session".to_string(), 300),
        ("seven_day", "Weekly".to_string(), 10080),
        ("seven_day_sonnet", weekly("Sonnet"), 10080),
        ("seven_day_opus", weekly("Opus"), 10080),
    ] {
        if let Some(limit) = window(
            &label,
            &body[key]["utilization"],
            &body[key]["resets_at"],
            Some(mins),
        ) {
            limits.push(limit);
        }
    }
    if let Some(scoped) = body["model_scoped"].as_array() {
        for data in scoped {
            let label = weekly(data["display_name"].as_str().unwrap_or("Model"));
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
pub fn fetch(client: &Client, homes: &[PathBuf]) -> Result<LiveData> {
    let provider = Provider::Claude;

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
