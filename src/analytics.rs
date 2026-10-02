use crate::{
    config::Config,
    model::{Provider, Snapshot, Tokens, UsageRecord},
    pricing::price_for,
};
use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone, Utc};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    Week,
    Month,
    Quarter,
}
impl Period {
    pub const ALL: [Self; 4] = [Self::Today, Self::Week, Self::Month, Self::Quarter];
    pub fn days(self) -> i64 {
        match self {
            Self::Today => 1,
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 90,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Week => "7 days",
            Self::Month => "30 days",
            Self::Quarter => "90 days",
        }
    }
    pub fn start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        let date = now.with_timezone(&Local).date_naive() - Duration::days(self.days() - 1);
        Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .unwrap_or_else(|| now.with_timezone(&Local))
            .with_timezone(&Utc)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Totals {
    pub tokens: Tokens,
    pub estimated: f64,
    pub reported: f64,
    pub reported_rows: usize,
    pub unpriced_rows: usize,
    pub cache_savings: f64,
    pub events: usize,
    pub sessions: usize,
}
#[derive(Debug, Clone)]
pub struct ModelRow {
    pub model: String,
    pub provider: Provider,
    pub totals: Totals,
}
#[derive(Debug, Clone)]
pub struct Bucket {
    pub date: NaiveDate,
    pub cost: [f64; 3],
    pub tokens: [u64; 3],
}
#[derive(Debug, Clone)]
pub struct Analysis {
    pub totals: Totals,
    pub providers: [Totals; 3],
    pub models: Vec<ModelRow>,
    pub buckets: Vec<Bucket>,
    pub previous_estimated: f64,
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
fn add(totals: &mut Totals, record: &UsageRecord, config: &Config) {
    totals.tokens.add(&record.tokens);
    totals.events += 1;
    if let Some(price) = price_for(&record.model, &config.prices) {
        totals.estimated += price.cost(&record.tokens);
        totals.cache_savings += price.cache_savings(&record.tokens);
    } else if record.tokens.total() > 0 {
        totals.unpriced_rows += 1;
    }
    if let Some(cost) = record.reported_cost {
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
    let first = period.start(now).with_timezone(&Local).date_naive();
    let start = period.start(now);
    let previous = start - Duration::days(period.days());
    let mut analysis = Analysis {
        totals: Totals::default(),
        providers: Default::default(),
        models: vec![],
        buckets: (0..period.days())
            .map(|d| Bucket {
                date: first + Duration::days(d),
                cost: [0.0; 3],
                tokens: [0; 3],
            })
            .collect(),
        previous_estimated: 0.0,
    };
    let mut models: BTreeMap<(Provider, String), Totals> = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    for record in &snapshot.records {
        if !config.enabled[record.provider.index()]
            || provider.is_some_and(|p| p != record.provider)
        {
            continue;
        }
        if record.timestamp >= previous
            && record.timestamp < start
            && let Some(price) = price_for(&record.model, &config.prices)
        {
            analysis.previous_estimated += price.cost(&record.tokens);
        }
        if !matches(record, period, provider, now) {
            continue;
        }
        add(&mut analysis.totals, record, config);
        add(
            &mut analysis.providers[record.provider.index()],
            record,
            config,
        );
        add(
            models
                .entry((record.provider, record.model.clone()))
                .or_default(),
            record,
            config,
        );
        if !record.session.is_empty() {
            sessions.insert((record.provider, record.session.clone()));
        }
        let day = (record.timestamp.with_timezone(&Local).date_naive() - first).num_days();
        if let Some(bucket) = usize::try_from(day)
            .ok()
            .and_then(|day| analysis.buckets.get_mut(day))
        {
            bucket.tokens[record.provider.index()] += record.tokens.total();
            if let Some(price) = price_for(&record.model, &config.prices) {
                bucket.cost[record.provider.index()] += price.cost(&record.tokens);
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
        .sort_by(|a, b| b.totals.estimated.total_cmp(&a.totals.estimated));
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
        "provider",
        "model",
        "project",
        "session",
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cache_write_tokens",
        "estimated_usd",
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
            price_for(&record.model, &config.prices)
                .map(|p| format!("{:.6}", p.cost(&record.tokens)))
                .unwrap_or_default(),
            record
                .reported_cost
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
