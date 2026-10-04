use crate::model::Provider;
#[cfg(not(windows))]
use crate::providers::http::keychain;
use crate::providers::{
    history::date,
    http::{response, token},
    local, proto,
};
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use reqwest::blocking::Client;
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::path::PathBuf;

// Installed-application OAuth identifiers published in the upstream client.
const GOOGLE_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";
struct OAuth {
    access: Option<String>,
    refresh: Option<String>,
    expiry: Option<DateTime<Utc>>,
}
fn oauth_json(raw: &str) -> Option<OAuth> {
    let raw = raw.trim().trim_start_matches('\u{feff}');
    let decoded = if let Some(encoded) = raw.strip_prefix("go-keyring-base64:") {
        Some(String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?)
    } else {
        None
    };
    let text = decoded.as_deref().unwrap_or(raw).trim();
    match serde_json::from_str::<Value>(text) {
        Ok(Value::String(access)) => bearer(&access),
        Ok(body) => oauth_object(&body, 0),
        Err(_) if text.starts_with(['{', '[']) => None,
        Err(_) => bearer(text),
    }
}
fn bearer(raw: &str) -> Option<OAuth> {
    let access = raw.strip_prefix("Bearer ").unwrap_or(raw).trim();
    if access.is_empty()
        || access == "Bearer"
        || access.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    Some(OAuth {
        access: Some(access.into()),
        refresh: None,
        expiry: None,
    })
}
fn oauth_object(body: &Value, depth: usize) -> Option<OAuth> {
    if depth >= 8 {
        return None;
    }
    let body = body
        .get("token")
        .filter(|token| token.is_object())
        .unwrap_or(body);
    let access = [
        "/access_token",
        "/accessToken",
        "/token",
        "/id_token",
        "/idToken",
        "/bearerToken",
        "/auth_token",
        "/authToken",
    ]
    .into_iter()
    .find_map(|path| token(body, path));
    let refresh = token(body, "/refresh_token").or_else(|| token(body, "/refreshToken"));
    if access.is_none() && refresh.is_none() {
        return ["tokens", "oauth", "oauth2", "credentials", "auth"]
            .into_iter()
            .find_map(|key| oauth_object(&body[key], depth + 1));
    }
    Some(OAuth {
        access,
        refresh,
        expiry: ["expiry", "expires_at", "expiresAt"]
            .into_iter()
            .find_map(|key| body[key].as_str().and_then(date)),
    })
}

#[cfg(windows)]
fn keyring_oauth() -> Option<OAuth> {
    use windows_sys::Win32::Security::Credentials::{CRED_TYPE_GENERIC, CredFree, CredReadW};
    // go-keyring stores this exact service/account as a generic UTF-8 credential.
    let target: Vec<u16> = "gemini:antigravity".encode_utf16().chain([0]).collect();
    let mut credential = std::ptr::null_mut();
    // SAFETY: target is NUL-terminated; CredReadW initializes the returned allocation.
    unsafe {
        if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0 {
            return None;
        }
        let value = &*credential;
        let result = if value.CredentialBlob.is_null() || value.CredentialBlobSize == 0 {
            None
        } else {
            let bytes =
                std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize);
            std::str::from_utf8(bytes).ok().and_then(oauth_json)
        };
        CredFree(credential.cast());
        result
    }
}
#[cfg(not(windows))]
fn keyring_oauth() -> Option<OAuth> {
    keychain("gemini", Some("antigravity")).and_then(|raw| oauth_json(&raw))
}
fn oauth_proto(raw: &str) -> Option<OAuth> {
    let outer = STANDARD.decode(raw.trim()).ok()?;
    let wrapper = proto::bytes(&outer, 1)?;
    if proto::text(wrapper, 1)?.as_str() != "oauthTokenInfoSentinelKey" {
        return None;
    }
    let payload = proto::bytes(wrapper, 2)?;
    let inner = STANDARD.decode(proto::text(payload, 1)?).ok()?;
    let access = proto::text(&inner, 1);
    let refresh = proto::text(&inner, 3);
    let expiry = proto::bytes(&inner, 4)
        .and_then(|stamp| proto::integer(stamp, 1))
        .and_then(|seconds| DateTime::from_timestamp(i64::try_from(seconds).ok()?, 0));
    if access.is_none() && refresh.is_none() {
        return None;
    }
    Some(OAuth {
        access,
        refresh,
        expiry,
    })
}
fn oauth(homes: &[PathBuf]) -> Result<Option<OAuth>> {
    // Current clients own their login in the OS keyring; SQLite is a legacy fallback.
    if let Some(auth) = keyring_oauth() {
        return Ok(Some(auth));
    }
    for home in homes {
        let file = home.join("antigravity-oauth-token");
        if file.exists() {
            let raw = std::fs::read_to_string(file)
                .map_err(|_| anyhow::anyhow!("Antigravity CLI login could not be read"))?;
            if let Some(auth) = oauth_json(&raw) {
                return Ok(Some(auth));
            }
        }
        let db = home.join("User/globalStorage/state.vscdb");
        if !db.exists() {
            continue;
        }
        let connection = local::database(&db)
            .map_err(|_| anyhow::anyhow!("Antigravity login could not be read"))?;
        if !local::table(&connection, "ItemTable")? {
            continue;
        }
        let raw: Option<String> = connection
            .query_row(
                "SELECT value FROM ItemTable WHERE key='antigravityUnifiedStateSync.oauthToken'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(auth) = raw.and_then(|raw| oauth_proto(&raw)) {
            return Ok(Some(auth));
        }
    }
    Ok(None)
}
pub(super) fn fingerprint(homes: &[PathBuf]) -> Result<Option<String>> {
    Ok(oauth(homes)?
        .and_then(|auth| auth.refresh.or(auth.access))
        .map(|value| crate::providers::history::hash(&value)))
}

pub(super) fn access(client: &Client, homes: &[PathBuf]) -> Result<String> {
    let auth =
        oauth(homes)?.context("Start Antigravity or run agy and sign in to read live limits")?;
    let access = crate::providers::auth::access(
        Provider::Antigravity,
        auth.access,
        auth.refresh,
        auth.expiry,
        |refresh| {
            let refreshed = response(
                client.post("https://oauth2.googleapis.com/token").form(&[
                    ("client_id", GOOGLE_CLIENT_ID),
                    ("client_secret", GOOGLE_CLIENT_SECRET),
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh),
                ]),
                Provider::Antigravity,
            )?;
            Ok((
                token(&refreshed, "/access_token").context("Sign in to Antigravity again")?,
                token(&refreshed, "/refresh_token"),
                refreshed["expires_in"].as_u64().unwrap_or(3600),
            ))
        },
    )?;
    Ok(access)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keyring_login_preserves_refresh_and_expiry() {
        let body = json!({"token":{"access_token":"fixture-access","refresh_token":"fixture-refresh","expiry":"2026-11-01T00:00:00Z"}}).to_string();
        for raw in [
            body.clone(),
            format!("go-keyring-base64:{}", STANDARD.encode(&body)),
        ] {
            let auth = oauth_json(&raw).unwrap();
            assert_eq!(auth.access.as_deref(), Some("fixture-access"));
            assert_eq!(auth.refresh.as_deref(), Some("fixture-refresh"));
            assert_eq!(auth.expiry.unwrap().timestamp(), 1793491200);
        }
        let auth = oauth_json(r#"{"credentials":{"accessToken":"fixture-access","refreshToken":"fixture-refresh","expiresAt":"2026-11-01T00:00:00Z"}}"#).unwrap();
        assert!(auth.expiry.is_some());
        assert_eq!(auth.refresh.as_deref(), Some("fixture-refresh"));
    }

    #[test]
    fn bearer_forms_are_supported_and_malformed_logins_are_rejected() {
        for raw in [
            "fixture-access",
            "Bearer fixture-access",
            "\"fixture-access\"",
        ] {
            assert_eq!(
                oauth_json(raw).unwrap().access.as_deref(),
                Some("fixture-access")
            );
        }
        for raw in [
            "",
            "{broken",
            "[broken",
            "{}",
            "null",
            "Bearer ",
            "go-keyring-base64:!",
        ] {
            assert!(
                oauth_json(raw).is_none(),
                "Invalid credential format must be rejected"
            );
        }
    }
}
