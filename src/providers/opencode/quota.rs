use crate::model::LimitWindow;
use crate::providers::http::window;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;

pub fn limits(body: &Value, captured_at: DateTime<Utc>) -> Result<Vec<LimitWindow>> {
    let usage = body
        .get("usage")
        .filter(|v| v.is_object())
        .context("OpenCode returned an unreadable usage response")?;
    let mut limits = vec![];
    for (key, label, minutes) in [
        ("rolling", "5-hour session", 300),
        ("weekly", "Weekly", 10080),
        ("monthly", "Monthly", 43200),
    ] {
        let data = &usage[key];
        let mut limit = window(label, &data["percent"], &data["resetsAt"], Some(minutes))
            .context("OpenCode did not report all Go usage windows")?;
        if key == "rolling"
            && limit.used_percent == 0.
            && limit
                .resets_at
                .is_some_and(|reset| ((reset - captured_at).num_seconds() - 18000).abs() <= 2)
        {
            limit.resets_at = None;
        }
        limits.push(limit);
    }
    Ok(limits)
}
