use crate::model::{CreditBalance, LimitWindow};
use crate::providers::{http::window, local};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct CachedLimits {
    pub limits: Vec<LimitWindow>,
    pub checked_at: DateTime<Utc>,
}

fn number(value: &Value, precise: &str, fallback: &str) -> Option<f64> {
    value
        .get(precise)
        .or_else(|| value.get(fallback))?
        .as_f64()
        .filter(|n| n.is_finite() && *n >= 0.)
}
fn credit_window(data: &Value, label: &str, reset: &Value) -> Option<LimitWindow> {
    let used = number(data, "currentUsageWithPrecision", "currentUsage")?;
    let limit = number(data, "usageLimitWithPrecision", "usageLimit")?;
    if limit <= 0. {
        return None;
    }
    let mut window = window(label, &json!(used / limit * 100.), reset, None)?;
    window.credits = Some(CreditBalance { used, limit });
    Some(window)
}
pub fn limits(body: &Value) -> Vec<LimitWindow> {
    let Some(buckets) = body
        .get("usageBreakdownList")
        .or_else(|| body.get("usageBreakdowns"))
        .and_then(Value::as_array)
    else {
        return vec![];
    };
    let mut limits = vec![];
    for data in buckets {
        let kind = data
            .get("resourceType")
            .or_else(|| data.get("type"))
            .and_then(Value::as_str);
        if kind != Some("CREDIT") && !(buckets.len() == 1 && kind.is_none()) {
            continue;
        }
        let reset = data.get("resetDate").unwrap_or(&body["nextDateReset"]);
        if let Some(limit) = credit_window(data, "Monthly credits", reset) {
            limits.push(limit);
        }
        let trial = &data["freeTrialUsage"];
        if trial["freeTrialStatus"]
            .as_str()
            .is_none_or(|status| status == "ACTIVE")
            && let Some(limit) = credit_window(trial, "Trial credits", &trial["expiryDate"])
        {
            limits.push(limit);
        }
        if let Some(bonuses) = data["bonuses"].as_array() {
            for bonus in bonuses {
                if bonus["status"]
                    .as_str()
                    .is_none_or(|status| status == "ACTIVE")
                    && let Some(limit) = credit_window(bonus, "Bonus credits", &bonus["expiryDate"])
                {
                    limits.push(limit);
                }
            }
        }
    }
    limits
}

pub fn cached_limits(homes: &[PathBuf]) -> Result<Option<CachedLimits>> {
    let mut newest = None;
    for home in homes {
        let path = home.join("User/globalStorage/state.vscdb");
        if !path.exists() {
            continue;
        }
        let connection = local::database(&path)?;
        let raw: Option<String> = connection
            .query_row(
                "SELECT value FROM ItemTable WHERE key='kiro.kiroAgent'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            continue;
        };
        let state: Value = serde_json::from_str(&raw)?;
        let state = &state["kiro.resourceNotifications.usageState"];
        let limits = limits(state);
        let checked_at = state["timestamp"]
            .as_i64()
            .and_then(DateTime::from_timestamp_millis);
        if !limits.is_empty()
            && let Some(checked_at) = checked_at
            && newest
                .as_ref()
                .is_none_or(|old: &CachedLimits| old.checked_at < checked_at)
        {
            newest = Some(CachedLimits { limits, checked_at });
        }
    }
    Ok(newest)
}
