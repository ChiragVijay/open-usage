//! Antigravity quotas and measured generation history, following OpenUsage's adapters.
mod auth;
mod history;
mod quota;
use crate::model::Provider;
use crate::providers::{
    http::{response, token},
    live::LiveData,
    process,
};
use anyhow::{Result, bail};
pub use history::{databases, generation, history, history_with_warnings};
use quota::plan;
pub use quota::{legacy, summary};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) fn fingerprint(homes: &[PathBuf]) -> Result<Option<String>> {
    auth::fingerprint(homes)
}

fn remote(client: &Client, method: &str, access: &str, body: &Value) -> Result<Value> {
    let mut last_error = None;
    for base in [
        "https://daily-cloudcode-pa.googleapis.com",
        "https://cloudcode-pa.googleapis.com",
    ] {
        match response(
            client
                .post(format!("{base}/v1internal:{method}"))
                .header("Accept", "application/json")
                .header(
                    "User-Agent",
                    if matches!(method, "loadCodeAssist" | "retrieveUserQuota") {
                        "agy"
                    } else {
                        "antigravity"
                    },
                )
                .bearer_auth(access)
                .json(body),
            Provider::Antigravity,
        ) {
            Ok(body) => return Ok(body),
            Err(error) => {
                if error.to_string().starts_with("Sign in") {
                    return Err(error);
                }
                last_error = Some(error);
            }
        }
    }
    Err(last_error.unwrap())
}
pub fn fetch(client: &Client, homes: &[PathBuf]) -> Result<LiveData> {
    // Self-signed certificates are allowed solely for fixed loopback requests.
    let local_client = Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_secs(1))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    for server in process::servers() {
        for port in server.ports {
            for scheme in ["https", "http"] {
                if Instant::now() >= deadline {
                    break;
                }
                let call = |method: &str| {
                    response(local_client.post(format!("{scheme}://127.0.0.1:{port}/exa.language_server_pb.LanguageServerService/{method}")).header("Connect-Protocol-Version","1").header("x-codeium-csrf-token",&server.csrf).json(&json!({"metadata":{"ideName":"antigravity","extensionName":"antigravity","ideVersion":"unknown","locale":"en"}})),Provider::Antigravity)
                };
                let quotas = call("RetrieveUserQuotaSummary")
                    .ok()
                    .and_then(|body| summary(&body));
                let status = call("GetUserStatus").ok();
                if let Some(limits) = quotas {
                    return Ok(LiveData {
                        limits,
                        plan: status.as_ref().and_then(plan),
                        records: vec![],
                    });
                }
                let limits = status
                    .as_ref()
                    .map(legacy)
                    .filter(|limits| !limits.is_empty())
                    .or_else(|| {
                        call("GetCommandModelConfigs")
                            .ok()
                            .map(|body| legacy(&body))
                    })
                    .unwrap_or_default();
                if !limits.is_empty() {
                    return Ok(LiveData {
                        limits,
                        plan: status.as_ref().and_then(plan),
                        records: vec![],
                    });
                }
            }
        }
    }
    let access = auth::access(client, homes)?;
    let tier = remote(client, "loadCodeAssist", &access, &json!({})).ok();
    if let Ok(body) = remote(client, "retrieveUserQuotaSummary", &access, &json!({}))
        && let Some(limits) = summary(&body)
    {
        return Ok(LiveData {
            limits,
            plan: tier.as_ref().and_then(plan),
            records: vec![],
        });
    }
    let models = remote(client, "fetchAvailableModels", &access, &json!({})).ok();
    let mut limits = models.as_ref().map(legacy).unwrap_or_default();
    if limits.is_empty() {
        let project = tier
            .as_ref()
            .and_then(|body| token(body, "/cloudaicompanionProject"));
        let body = if let Some(project) = project {
            json!({"project":project})
        } else {
            json!({})
        };
        limits = legacy(&remote(client, "retrieveUserQuota", &access, &body)?);
    }
    if limits.is_empty() {
        bail!("Antigravity did not report quota limits");
    }
    Ok(LiveData {
        limits,
        plan: tier.as_ref().and_then(plan),
        records: vec![],
    })
}
