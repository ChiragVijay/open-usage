use crate::model::Provider;
use crate::providers::{
    history::date,
    http::{keychain, response, token},
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
    let decoded = raw
        .strip_prefix("go-keyring-base64:")
        .and_then(|raw| STANDARD.decode(raw).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());
    let body: Value = serde_json::from_str(decoded.as_deref().unwrap_or(raw)).ok()?;
    let body = body
        .get("token")
        .filter(|token| token.is_object())
        .unwrap_or(&body);
    let access = token(body, "/access_token").or_else(|| token(body, "/accessToken"));
    let refresh = token(body, "/refresh_token").or_else(|| token(body, "/refreshToken"));
    if access.is_none() && refresh.is_none() {
        return None;
    }
    Some(OAuth {
        access,
        refresh,
        expiry: body["expiry"].as_str().and_then(date),
    })
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
    for home in homes {
        let file = home.join("antigravity-oauth-token");
        if file.exists() {
            return std::fs::read_to_string(file)
                .map(|raw| oauth_json(&raw))
                .map_err(|_| anyhow::anyhow!("Antigravity CLI login could not be read"));
        }
        let db = home.join("User/globalStorage/state.vscdb");
        if !db.exists() {
            continue;
        }
        let connection = local::database(&db)
            .map_err(|_| anyhow::anyhow!("Antigravity login could not be read"))?;
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
    Ok(keychain("gemini", Some("antigravity")).and_then(|raw| oauth_json(&raw)))
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
