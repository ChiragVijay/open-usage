use crate::model::Tokens;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}
impl Price {
    pub fn valid(&self) -> bool {
        [self.input, self.output, self.cache_read, self.cache_write]
            .into_iter()
            .all(|v| v.is_finite() && v >= 0.0)
    }
    pub fn cost(&self, tokens: &Tokens) -> f64 {
        (tokens.input as f64 * self.input
            + tokens.output as f64 * self.output
            + tokens.cache_read as f64 * self.cache_read
            + tokens.cache_write as f64 * self.cache_write)
            / 1_000_000.0
    }
    pub fn cache_savings(&self, tokens: &Tokens) -> f64 {
        tokens.cache_read as f64 * (self.input - self.cache_read).max(0.0) / 1_000_000.0
    }
}
/// A deliberately small, documented baseline. Unknown models remain unpriced.
pub fn price_for(model: &str, overrides: &BTreeMap<String, Price>) -> Option<Price> {
    if let Some(price) = overrides.get(model) {
        return Some(price.clone());
    }
    let model = model.to_lowercase().replace('.', "-");
    let (input, output, read, write) =
        if model == "gpt-6-1-sol" || model.starts_with("gpt-6-1-sol-") {
            (2.0, 10.0, 0.10, 2.50)
        } else if model == "gpt-6-sol" || model.starts_with("gpt-6-sol-") {
            (2.0, 10.0, 0.20, 2.50)
        } else if model == "gpt-5-6-terra" || model.starts_with("gpt-5-6-terra-") {
            (2.0, 12.0, 0.20, 2.50)
        } else if model == "gpt-5-6-luna" || model.starts_with("gpt-5-6-luna-") {
            (0.20, 1.20, 0.02, 0.25)
        } else if model == "gpt-6-astra" || model.starts_with("gpt-6-astra-") {
            (10.0, 50.0, 1.0, 12.50)
        } else if model == "gpt-6-luna" || model.starts_with("gpt-6-luna-") {
            (0.10, 0.50, 0.01, 0.125)
        } else if model == "gpt-5-6-sol" || model.starts_with("gpt-5-6-sol-") {
            (4.0, 20.0, 0.40, 5.0)
        } else if model.starts_with("claude-sonnet-5") {
            (2.0, 10.0, 0.20, 2.50)
        } else if model.starts_with("claude-opus-5-5") {
            (4.0, 20.0, 0.20, 5.0)
        } else if model.starts_with("claude-fable-5-1") || model.starts_with("claude-mythos-5-1") {
            (10.0, 50.0, 0.25, 12.50)
        } else if model.starts_with("claude-opus-5")
            || model.starts_with("claude-opus-4-7")
            || model.starts_with("claude-opus-4-8")
        {
            (5.0, 25.0, 0.50, 6.25)
        } else if model.starts_with("claude-sonnet-4") {
            (3.0, 15.0, 0.30, 3.75)
        } else if model.starts_with("claude-opus-4-5") || model.starts_with("claude-opus-4-6") {
            (5.0, 25.0, 0.50, 6.25)
        } else if model == "claude-opus-4"
            || model.starts_with("claude-opus-4-1")
            || model.starts_with("claude-opus-4-202")
        {
            (15.0, 75.0, 1.50, 18.75)
        } else if model.starts_with("claude-haiku-4-5") {
            (1.0, 5.0, 0.10, 1.25)
        } else if model == "gpt-5-2"
            || model.starts_with("gpt-5-2-")
            || model == "gpt-5-3-codex"
            || model.starts_with("gpt-5-3-codex-")
        {
            (1.75, 14.0, 0.175, 1.75)
        } else if model == "gpt-5-4" || model == "gpt-5-4-codex" {
            (2.5, 15.0, 0.25, 2.5)
        } else if model == "gpt-5-mini" {
            (0.25, 2.0, 0.025, 0.25)
        } else if model == "gpt-5"
            || model == "gpt-5-codex"
            || model == "gpt-5-1"
            || model == "gpt-5-1-codex"
            || model == "gpt-5-1-codex-max"
        {
            (1.25, 10.0, 0.125, 1.25)
        } else {
            return None;
        };
    Some(Price {
        input,
        output,
        cache_read: read,
        cache_write: write,
    })
}

pub fn import_prices(path: &Path) -> Result<BTreeMap<String, Price>> {
    #[derive(Deserialize)]
    struct Row {
        model: String,
        input: f64,
        output: f64,
        cache_read: Option<f64>,
        cache_write: Option<f64>,
    }
    let mut prices = BTreeMap::new();
    for row in csv::Reader::from_path(path)?.deserialize::<Row>() {
        let row = row?;
        let price = Price {
            input: row.input,
            output: row.output,
            cache_read: row.cache_read.unwrap_or(row.input),
            cache_write: row.cache_write.unwrap_or(row.input),
        };
        if row.model.trim().is_empty() || !price.valid() {
            bail!("Every model needs a name and nonnegative USD rates");
        }
        prices.insert(row.model, price);
    }
    Ok(prices)
}
