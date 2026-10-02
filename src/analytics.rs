use crate::{
    config::Config,
    model::{Provider, Snapshot, Tokens, UsageRecord},
};
use anyhow::Result;
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    TwoHours,
    Day,
    Week,
    Month,
    Quarter,
}
impl Period {
    pub const ALL: [Self; 5] = [
        Self::TwoHours,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Quarter,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::TwoHours => "2h",
            Self::Day => "24h",
            Self::Week => "7d",
            Self::Month => "30d",
            Self::Quarter => "90d",
        }
    }
    pub fn days(self) -> i64 {
        match self {
            Self::Today | Self::TwoHours | Self::Day => 1,
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::TwoHours => "Last 2 hours",
            Self::Day => "Last 24 hours",
            Self::Week => "7 days",
            Self::Month => "30 days",
            Self::Quarter => "90 days",
        }
    }
    pub fn start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        if self != Self::Today {
            return now
                - match self {
                    Self::TwoHours => Duration::hours(2),
                    Self::Day => Duration::hours(24),
                    _ => Duration::days(self.days()),
                };
        }
        let date = now.with_timezone(&Local).date_naive();
        Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .unwrap_or_else(|| now.with_timezone(&Local))
            .with_timezone(&Utc)
    }
    pub fn bucket_seconds(self) -> i64 {
        match self {
            Self::TwoHours => 300,
            Self::Today | Self::Day => 3600,
            _ => 86400,
        }
    }
    pub fn chart_title(self) -> &'static str {
        match self {
            Self::TwoHours => "Usage · 5-minute intervals",
            Self::Today | Self::Day => "Usage · hourly",
            _ => "Usage · daily",
        }
    }
    pub fn bucket_label(self, at: DateTime<Utc>) -> String {
        at.with_timezone(&Local)
            .format(if self.bucket_seconds() < 86400 {
                "%H:%M"
            } else {
                "%b %-d"
            })
            .to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Totals {
    pub tokens: Tokens,
    pub reported: f64,
    pub reported_rows: usize,
    pub events: usize,
    pub sessions: usize,
}
impl Totals {
    /// None means the account did not report cost, rather than zero spend.
    pub fn spend(&self) -> Option<f64> {
        (self.reported_rows > 0).then_some(self.reported)
    }
    pub fn cache_percent(&self) -> f64 {
        let total = self.tokens.total();
        if total == 0 {
            0.0
        } else {
            self.tokens.cache_read as f64 / total as f64 * 100.0
        }
    }
}
#[derive(Debug, Clone)]
pub struct ModelRow {
    pub model: String,
    pub provider: Provider,
    pub totals: Totals,
}
#[derive(Debug, Clone)]
pub struct Bucket {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub cost: [f64; 3],
    pub cost_rows: [usize; 3],
    pub tokens: [u64; 3],
}
#[derive(Debug, Clone)]
pub struct Analysis {
    pub totals: Totals,
    pub providers: [Totals; 3],
    pub models: Vec<ModelRow>,
    pub buckets: Vec<Bucket>,
}
pub fn matches(
    record: &UsageRecord,
    period: Period,
    provider: Option<Provider>,
    now: DateTime<Utc>,
) -> bool {
    record.timestamp >= period.start(now)
        && record.timestamp <= now
        && provider.is_none_or(|p| p == record.provider)
}
fn add(totals: &mut Totals, record: &UsageRecord) {
    totals.tokens.add(&record.tokens);
    totals.events += 1;
    if let Some(cost) = record
        .reported_cost
        .filter(|cost| cost.is_finite() && *cost >= 0.0)
    {
        totals.reported += cost;
        totals.reported_rows += 1;
    }
}
pub fn analyze(
    snapshot: &Snapshot,
    config: &Config,
    period: Period,
    provider: Option<Provider>,
    now: DateTime<Utc>,
) -> Analysis {
    let first = period.start(now);
    let interval = period.bucket_seconds();
    let bucket_count = ((now - first).num_seconds().max(1) + interval - 1) / interval;
    let mut analysis = Analysis {
        totals: Totals::default(),
        providers: Default::default(),
        models: vec![],
        buckets: (0..bucket_count)
            .map(|d| Bucket {
                start: first + Duration::seconds(d * interval),
                end: (first + Duration::seconds((d + 1) * interval)).min(now),
                cost: [0.0; 3],
                cost_rows: [0; 3],
                tokens: [0; 3],
            })
            .collect(),
    };
    let mut models: BTreeMap<(Provider, String), Totals> = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    for record in &snapshot.records {
        if !config.enabled[record.provider.index()]
            || provider.is_some_and(|p| p != record.provider)
        {
            continue;
        }
        if !matches(record, period, provider, now) {
            continue;
        }
        add(&mut analysis.totals, record);
        add(&mut analysis.providers[record.provider.index()], record);
        add(
            models
                .entry((record.provider, record.model.clone()))
                .or_default(),
            record,
        );
        if !record.session.is_empty() {
            sessions.insert((record.provider, record.session.clone()));
        }
        let index = ((record.timestamp - first).num_seconds() / interval).min(bucket_count - 1);
        if let Some(bucket) = usize::try_from(index)
            .ok()
            .and_then(|index| analysis.buckets.get_mut(index))
        {
            bucket.tokens[record.provider.index()] =
                bucket.tokens[record.provider.index()].saturating_add(record.tokens.total());
            if let Some(cost) = record
                .reported_cost
                .filter(|cost| cost.is_finite() && *cost >= 0.0)
            {
                bucket.cost[record.provider.index()] += cost;
                bucket.cost_rows[record.provider.index()] += 1;
            }
        }
    }
    analysis.totals.sessions = sessions.len();
    for (provider, _) in sessions {
        analysis.providers[provider.index()].sessions += 1;
    }
    analysis.models = models
        .into_iter()
        .map(|((provider, model), totals)| ModelRow {
            model,
            provider,
            totals,
        })
        .collect();
    analysis
        .models
        .sort_by_key(|row| std::cmp::Reverse(row.totals.tokens.total()));
    analysis
}
pub fn export_csv(
    path: &Path,
    snapshot: &Snapshot,
    config: &Config,
    period: Period,
    provider: Option<Provider>,
) -> Result<usize> {
    let mut writer = csv::Writer::from_path(path)?;
    writer.write_record([
        "timestamp",
        "account",
        "model",
        "project",
        "session",
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
        "reported_usd",
    ])?;
    let mut count = 0;
    for record in &snapshot.records {
        if !config.enabled[record.provider.index()]
            || !matches(record, period, provider, Utc::now())
        {
            continue;
        }
        writer.write_record([
            record.timestamp.to_rfc3339(),
            record.provider.key().into(),
            record.model.clone(),
            record.project.clone(),
            record.session.clone(),
            record.tokens.input.to_string(),
            record.tokens.output.to_string(),
            record.tokens.cache_read.to_string(),
            record.tokens.cache_write.to_string(),
            record
                .reported_cost
                .filter(|cost| cost.is_finite() && *cost >= 0.0)
                .map(|p| format!("{p:.6}"))
                .unwrap_or_default(),
        ])?;
        count += 1;
    }
    writer.flush()?;
    Ok(count)
}
pub fn compact(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2}B", value as f64 / 1e9)
    } else if value >= 1_000_000 {
        format!("{:.2}M", value as f64 / 1e6)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1e3)
    } else {
        value.to_string()
    }
}
