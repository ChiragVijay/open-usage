//! API-equivalent token pricing.
//!
//! ChatGPT session logs record tokens and no dollar amount. Rates come from
//! LiteLLM's public model table, the same source t3code prices against. A
//! provider-reported cost always wins. The result is an API-equivalent estimate,
//! not a subscription invoice.

use crate::{config::data_dir, model::UsageRecord};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    sync::{Mutex, OnceLock, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const RATES_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";
const FRESH_FOR: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy)]
struct ModelRate {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
}

#[derive(Serialize, Deserialize)]
struct RatesCache {
    fetched_at_ms: i64,
    document: Value,
}

static TABLE: OnceLock<RwLock<HashMap<String, ModelRate>>> = OnceLock::new();
static FETCH: OnceLock<Mutex<()>> = OnceLock::new();
static UPDATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static STATE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static FETCHED_AT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Whether the LiteLLM rate table is available for estimates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RateState {
    Loading,
    Ready,
    Unavailable,
}

fn table() -> &'static RwLock<HashMap<String, ModelRate>> {
    TABLE.get_or_init(|| RwLock::new(HashMap::new()))
}
fn fetch_lock() -> &'static Mutex<()> {
    FETCH.get_or_init(|| Mutex::new(()))
}

/// Loads cached rates, then downloads a new table when the saved copy is older than a day.
pub fn start() {
    std::thread::spawn(|| {
        let _ = refresh_once();
    });
}

/// Loads rates before returning. Used by the CLI.
pub fn prepare() {
    let _ = refresh_once();
}

pub fn take_updated() -> bool {
    UPDATED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

pub fn state() -> RateState {
    match STATE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => RateState::Ready,
        2 => RateState::Unavailable,
        _ => RateState::Loading,
    }
}

pub fn estimate(record: &UsageRecord) -> Option<f64> {
    let key = normalize(&record.model);
    let bare = bare_name(&key);
    if bare.is_empty() || unpriceable(bare) {
        return None;
    }
    let table = table().read().ok()?;
    let rate = lookup(&table, &key)?;
    Some(
        record.tokens.input as f64 * rate.input
            + record.tokens.cache_read as f64 * rate.cache_read
            + record.tokens.cache_write as f64 * rate.cache_write
            + record.tokens.output as f64 * rate.output,
    )
}

fn refresh_once() -> Result<()> {
    let _guard = fetch_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if rates_are_fresh() {
        return Ok(());
    }
    if let Some((fetched_at, document)) = read_cache(true).ok().flatten() {
        install(parse_rate_table(&document));
        remember(fetched_at);
        return Ok(());
    }
    match fetch_document() {
        Ok(document) => {
            let fetched_at = write_cache(&document).unwrap_or_else(|_| now_ms());
            install(parse_rate_table(&document));
            remember(fetched_at);
        }
        Err(_) => {
            if let Some((fetched_at, document)) = read_cache(false).ok().flatten() {
                install(parse_rate_table(&document));
                remember(fetched_at);
            } else {
                mark(RateState::Unavailable);
            }
        }
    }
    Ok(())
}

fn rates_are_fresh() -> bool {
    let fetched_at = FETCHED_AT.load(std::sync::atomic::Ordering::Relaxed);
    fetched_at > 0
        && state() == RateState::Ready
        && now_ms().saturating_sub(fetched_at) <= FRESH_FOR.as_millis() as i64
}

fn remember(fetched_at: i64) {
    FETCHED_AT.store(fetched_at, std::sync::atomic::Ordering::Relaxed);
}

fn install(rates: HashMap<String, ModelRate>) {
    if rates.is_empty() {
        mark(RateState::Unavailable);
        return;
    }
    *table().write().unwrap_or_else(|poison| poison.into_inner()) = rates;
    mark(RateState::Ready);
}

fn mark(state: RateState) {
    STATE.store(
        match state {
            RateState::Loading => 0,
            RateState::Ready => 1,
            RateState::Unavailable => 2,
        },
        std::sync::atomic::Ordering::Relaxed,
    );
    UPDATED.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn cache_path() -> std::path::PathBuf {
    data_dir().join("usage-model-rates.json")
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn read_cache(fresh_only: bool) -> Result<Option<(i64, Value)>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(None);
    }
    let cache: RatesCache = serde_json::from_slice(&fs::read(&path)?)?;
    if fresh_only && now_ms().saturating_sub(cache.fetched_at_ms) > FRESH_FOR.as_millis() as i64 {
        return Ok(None);
    }
    Ok(Some((cache.fetched_at_ms, cache.document)))
}

fn write_cache(document: &Value) -> Result<i64> {
    fs::create_dir_all(data_dir())?;
    let fetched_at = now_ms();
    let cache = RatesCache {
        fetched_at_ms: fetched_at,
        document: document.clone(),
    };
    fs::write(cache_path(), serde_json::to_vec(&cache)?)?;
    Ok(fetched_at)
}

fn fetch_document() -> Result<Value> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        .build()?;
    let document = client
        .get(RATES_URL)
        .header("Accept", "application/json")
        .send()
        .context("model rates could not be downloaded")?
        .error_for_status()?
        .json::<Value>()?;
    Ok(document)
}

fn parse_rate_table(document: &Value) -> HashMap<String, ModelRate> {
    let mut table = HashMap::new();
    let Some(entries) = document.as_object() else {
        return table;
    };
    for (name, raw) in entries {
        let Some(entry) = raw.as_object() else {
            continue;
        };
        let (Some(input), Some(output)) = (
            finite(entry.get("input_cost_per_token")),
            finite(entry.get("output_cost_per_token")),
        ) else {
            continue;
        };
        let key = normalize(name);
        if key.is_empty() {
            continue;
        }
        table.insert(
            key,
            ModelRate {
                input,
                output,
                cache_read: finite(entry.get("cache_read_input_token_cost")).unwrap_or(input),
                cache_write: finite(entry.get("cache_creation_input_token_cost")).unwrap_or(input),
            },
        );
    }
    let mut aliases: HashMap<String, Option<ModelRate>> = HashMap::new();
    for (key, rate) in &table {
        let alias = bare_name(key);
        if alias.is_empty() || alias == key || table.contains_key(alias) {
            continue;
        }
        match aliases.get(alias) {
            None => {
                aliases.insert(alias.to_owned(), Some(*rate));
            }
            Some(Some(held)) if same_rate(held, rate) => {}
            Some(_) => {
                aliases.insert(alias.to_owned(), None);
            }
        }
    }
    for (alias, rate) in aliases {
        if let Some(rate) = rate {
            table.insert(alias, rate);
        }
    }
    table
}

fn lookup<'a>(table: &'a HashMap<String, ModelRate>, model: &str) -> Option<&'a ModelRate> {
    let key = normalize(model);
    if let Some(rate) = table.get(&key) {
        return Some(rate);
    }
    for name in [key.as_str(), bare_name(&key)] {
        if let Some(rest) = name.strip_prefix("gpt-daybreak-") {
            let alias = format!("daybreak-{rest}");
            if let Some(rate) = table.get(&alias) {
                return Some(rate);
            }
        }
    }
    None
}

fn same_rate(a: &ModelRate, b: &ModelRate) -> bool {
    a.input == b.input
        && a.output == b.output
        && a.cache_read == b.cache_read
        && a.cache_write == b.cache_write
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|rate| rate.is_finite() && *rate >= 0.0)
}

fn normalize(model: &str) -> String {
    let trimmed = model.trim().to_ascii_lowercase();
    match trimmed.find('[') {
        Some(index) => trimmed[..index].to_owned(),
        None => trimmed,
    }
}

fn bare_name(key: &str) -> &str {
    key.rsplit_once('/').map(|(_, name)| name).unwrap_or(key)
}

fn unpriceable(bare: &str) -> bool {
    matches!(
        bare,
        "<synthetic>" | "synthetic" | "opus" | "sonnet" | "haiku" | "fable"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prices_uncached_cached_and_output_tokens() {
        let table = parse_rate_table(&json!({
            "gpt-5.4": {
                "input_cost_per_token": 0.0000025,
                "output_cost_per_token": 0.000015,
                "cache_read_input_token_cost": 0.00000025
            }
        }));
        let rate = lookup(&table, "gpt-5.4").unwrap();
        let cost = 1_000_000.0 * rate.input
            + 1_000_000.0 * rate.cache_read
            + 0.0
            + 1_000_000.0 * rate.output;
        assert!((cost - 17.75).abs() < 1e-9);
        assert!(lookup(&table, "GPT-5.4").is_some());
    }

    #[test]
    fn keeps_provider_keys_distinct_and_aliases_only_when_rates_agree() {
        let table = parse_rate_table(&json!({
            "gpt-5": {"input_cost_per_token": 1.0, "output_cost_per_token": 2.0, "cache_read_input_token_cost": 0.1},
            "replicate/openai/gpt-5": {"input_cost_per_token": 9.0, "output_cost_per_token": 9.0},
            "openai/other-model": {"input_cost_per_token": 3.0, "output_cost_per_token": 4.0},
            "together/other-model": {"input_cost_per_token": 8.0, "output_cost_per_token": 8.0}
        }));
        assert_eq!(lookup(&table, "gpt-5").unwrap().cache_read, 0.1);
        assert_eq!(lookup(&table, "replicate/openai/gpt-5").unwrap().input, 9.0);
        assert!(lookup(&table, "other-model").is_none());
    }

    #[test]
    fn maps_daybreak_aliases_and_skips_ambiguous_family_names() {
        let table = parse_rate_table(&json!({
            "daybreak-blue-latest": {"input_cost_per_token": 1.0, "output_cost_per_token": 2.0}
        }));
        assert!(lookup(&table, "gpt-daybreak-blue-latest").is_some());
        assert!(lookup(&table, "opus").is_none() || unpriceable("opus"));
    }

    #[test]
    fn drops_half_specified_rates() {
        let table = parse_rate_table(&json!({
            "partial": {"input_cost_per_token": 1.0}
        }));
        assert!(table.is_empty());
    }
}
