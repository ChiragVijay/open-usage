use super::{
    http::{read, response, token, window},
    live::LiveData,
};
use crate::model::{LimitWindow, Provider};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::path::PathBuf;

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
pub fn fetch(client: &Client, homes: &[PathBuf]) -> Result<LiveData> {
    let provider = Provider::Codex;

    let auth = homes
                .iter()
                .find_map(|home| read(&home.join("auth.json")))
                .filter(|auth| token(auth,"/tokens/access_token").is_some())
                .or_else(|| super::opencode::codex_auth(homes).ok().flatten().map(|auth| json!({"tokens":{"access_token":auth["access"],"account_id":auth["accountId"]}})))
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
