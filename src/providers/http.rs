//! Fixed-endpoint HTTP, credential reads and quota parsing shared by adapters.
use super::history::date;
use crate::model::{LimitWindow, Provider};
use anyhow::{Result, anyhow, bail};
use chrono::DateTime;
use reqwest::blocking::RequestBuilder;
use serde_json::Value;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
use std::{fs, path::Path};

pub(super) fn read(path: &Path) -> Option<Value> {
    fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
}
pub(super) fn token(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
}

#[cfg(target_os = "macos")]
pub(super) fn keychain(service: &str, account: Option<&str>) -> Option<String> {
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
pub(super) fn keychain(_service: &str, _account: Option<&str>) -> Option<String> {
    None
}

pub(super) fn response(request: RequestBuilder, provider: Provider) -> Result<Value> {
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
pub(super) fn window(
    label: &str,
    used: &Value,
    reset: &Value,
    duration: Option<u64>,
) -> Option<LimitWindow> {
    let used = used.as_f64().filter(|v| v.is_finite())?;
    let resets_at = reset.as_str().and_then(date).or_else(|| {
        reset
            .as_i64()
            .or_else(|| {
                reset
                    .as_f64()
                    .filter(|at| at.is_finite() && *at >= 0. && *at < i64::MAX as f64)
                    .map(|at| at as i64)
            })
            .and_then(|at| {
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
        credits: None,
    })
}
