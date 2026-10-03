use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Codex,
    Claude,
    Cursor,
    Kiro,
    Antigravity,
    OpenCode,
}

impl Provider {
    pub const COUNT: usize = 6;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Codex,
        Self::Claude,
        Self::Cursor,
        Self::Kiro,
        Self::Antigravity,
        Self::OpenCode,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "ChatGPT",
            Self::Claude => "Claude",
            Self::Cursor => "Cursor",
            Self::Kiro => "Kiro",
            Self::Antigravity => "Antigravity",
            Self::OpenCode => "OpenCode",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Cursor => "cursor",
            Self::Kiro => "kiro",
            Self::Antigravity => "antigravity",
            Self::OpenCode => "opencode",
        }
    }
    pub fn account_name(self) -> &'static str {
        match self {
            Self::Codex => "ChatGPT",
            Self::Claude => "Claude Code",
            Self::Cursor => "Cursor",
            Self::Kiro => "Kiro",
            Self::Antigravity => "Antigravity",
            Self::OpenCode => "OpenCode",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Codex => 0,
            Self::Claude => 1,
            Self::Cursor => 2,
            Self::Kiro => 3,
            Self::Antigravity => 4,
            Self::OpenCode => 5,
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
    /// Usage cost reported by this account's provider. Never inferred from a model price.
    pub reported_cost: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditBalance {
    pub used: f64,
    pub limit: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitWindow {
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<CreditBalance>,
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
    #[serde(default)]
    pub detected: bool,
    #[serde(default)]
    pub installations: Vec<String>,
    pub sources: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_fingerprint: Option<String>,
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
            detected: false,
            installations: vec![],
            sources: 0,
            account_fingerprint: None,
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
impl Snapshot {
    /// Older caches may lack newly supported providers or use a different order.
    pub fn normalize_providers(&mut self) {
        let mut statuses: std::collections::BTreeMap<_, _> = std::mem::take(&mut self.providers)
            .into_iter()
            .map(|status| (status.provider, status))
            .collect();
        self.providers = Provider::ALL
            .into_iter()
            .map(|provider| {
                statuses
                    .remove(&provider)
                    .unwrap_or_else(|| ProviderStatus::empty(provider))
            })
            .collect();
    }
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
