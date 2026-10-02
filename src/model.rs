use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Codex,
    Claude,
    Cursor,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Cursor];
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Codex => 0,
            Self::Claude => 1,
            Self::Cursor => 2,
        }
    }
}

/// Input excludes cache reads/writes. Output includes reasoning, where reported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}
impl Tokens {
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
    pub fn add(&mut self, other: &Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub provider: Provider,
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub session: String,
    pub project: String,
    pub tokens: Tokens,
    /// Account-reported cost, independent of API-equivalent estimates.
    pub reported_cost: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitWindow {
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u64>,
}
impl LimitWindow {
    pub fn remaining(&self) -> f64 {
        (100.0 - self.used_percent).clamp(0.0, 100.0)
    }
    pub fn expired(&self, now: DateTime<Utc>) -> bool {
        self.resets_at.is_some_and(|at| at <= now)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider: Provider,
    pub detail: String,
    pub plan: Option<String>,
    pub limits: Vec<LimitWindow>,
    pub checked_at: Option<DateTime<Utc>>,
    pub stale: bool,
    pub connected: bool,
    pub sources: usize,
}
impl ProviderStatus {
    pub fn empty(provider: Provider) -> Self {
        Self {
            provider,
            detail: "Connect a source to see usage".into(),
            plan: None,
            limits: vec![],
            checked_at: None,
            stale: true,
            connected: false,
            sources: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub records: Vec<UsageRecord>,
    pub providers: Vec<ProviderStatus>,
    pub refreshed_at: Option<DateTime<Utc>>,
    pub notices: Vec<String>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            records: vec![],
            providers: Provider::ALL
                .into_iter()
                .map(ProviderStatus::empty)
                .collect(),
            refreshed_at: None,
            notices: vec![],
        }
    }
}
