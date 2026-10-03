//! Session-only derived tokens. Provider-owned credentials are never changed.
use super::history::hash;
use crate::model::Provider;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};
struct Derived {
    identity: String,
    access: String,
    refresh: String,
    expiry: DateTime<Utc>,
}
static TOKENS: OnceLock<Mutex<BTreeMap<Provider, Derived>>> = OnceLock::new();
pub fn access(
    provider: Provider,
    access: Option<String>,
    refresh: Option<String>,
    expiry: Option<DateTime<Utc>>,
    renew: impl FnOnce(&str) -> Result<(String, Option<String>, u64)>,
) -> Result<String> {
    // The current login is required on every call; cached tokens cannot revive logout.
    let identity = hash(
        refresh
            .as_deref()
            .or(access.as_deref())
            .context("Sign in again to read usage limits")?,
    );
    let mut tokens = TOKENS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(cached) = tokens.get(&provider)
        && cached.identity == identity
        && cached.expiry > Utc::now() + chrono::Duration::seconds(60)
        && expiry.is_none_or(|expiry| expiry <= cached.expiry)
    {
        return Ok(cached.access.clone());
    }
    if let Some(access) = access
        && expiry.is_none_or(|expiry| expiry > Utc::now() + chrono::Duration::seconds(60))
    {
        tokens.remove(&provider);
        return Ok(access);
    }
    let refresh = tokens
        .get(&provider)
        .filter(|cached| cached.identity == identity)
        .map(|cached| cached.refresh.clone())
        .or(refresh)
        .context("Sign in again to renew access")?;
    drop(tokens);
    let (access, rotated, seconds) = renew(&refresh)?;
    anyhow::ensure!(
        !access.trim().is_empty() && (1..=86400).contains(&seconds),
        "Provider returned an unreadable token renewal"
    );
    TOKENS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(
            provider,
            Derived {
                identity,
                access: access.clone(),
                refresh: rotated.unwrap_or(refresh),
                expiry: Utc::now() + chrono::Duration::seconds(seconds as i64),
            },
        );
    Ok(access)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derived_tokens_require_current_login_and_do_not_cross_accounts() {
        let expired = Utc::now() - chrono::Duration::hours(1);
        let first = access(
            Provider::OpenCode,
            Some("old".into()),
            Some("account-one".into()),
            Some(expired),
            |_| Ok(("derived-one".into(), Some("rotated".into()), 3600)),
        )
        .unwrap();
        assert_eq!(first, "derived-one");
        assert_eq!(
            access(
                Provider::OpenCode,
                Some("old".into()),
                Some("account-one".into()),
                Some(expired),
                |_| panic!("must reuse")
            )
            .unwrap(),
            first
        );
        assert!(
            access(Provider::OpenCode, None, None, None, |_| panic!(
                "logged out"
            ))
            .is_err()
        );
        assert_eq!(
            access(
                Provider::OpenCode,
                Some("fresh-two".into()),
                Some("account-two".into()),
                None,
                |_| panic!("must use current login")
            )
            .unwrap(),
            "fresh-two"
        );
    }
}
