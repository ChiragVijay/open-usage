use crate::model::*;
use chrono::{Duration, Utc};

pub fn snapshot() -> Snapshot {
    let now = Utc::now();
    let mut records = vec![];
    for day in 0..90 {
        for provider in Provider::ALL {
            let count = 2 + ((day * 7 + provider.index() as i64 * 3) % 8);
            for event in 0..count {
                let scale = (3 + ((day * 13 + event * 17 + provider.index() as i64) % 14)) as u64;
                let model = match provider {
                    Provider::Codex => "gpt-5.4",
                    Provider::Claude => {
                        if event % 4 == 0 {
                            "claude-opus-4-6"
                        } else {
                            "claude-sonnet-4-6"
                        }
                    }
                    Provider::Cursor => "claude-sonnet-4-6",
                };
                records.push(UsageRecord {
                    id: format!("demo-{day}-{provider:?}-{event}"),
                    provider,
                    timestamp: now - Duration::days(day) - Duration::minutes(event * 41 + 4),
                    model: model.into(),
                    session: format!("{day}-{event}"),
                    project: ["open-usage", "atlas", "design-system"][event as usize % 3].into(),
                    tokens: Tokens {
                        input: scale * 2100,
                        output: scale * 720,
                        cache_read: scale * 19500,
                        cache_write: if provider == Provider::Claude {
                            scale * 1500
                        } else {
                            0
                        },
                    },
                    reported_cost: if provider == Provider::Cursor {
                        Some(scale as f64 * 0.12)
                    } else {
                        None
                    },
                    source: "Example data".into(),
                });
            }
        }
    }
    records.sort_by_key(|record| std::cmp::Reverse(record.timestamp));
    let providers = Provider::ALL
        .into_iter()
        .map(|provider| ProviderStatus {
            provider,
            detail: "Example account · not live data".into(),
            plan: Some(
                match provider {
                    Provider::Codex => "ChatGPT Pro",
                    Provider::Claude => "Claude Max",
                    Provider::Cursor => "Cursor Pro",
                }
                .into(),
            ),
            limits: vec![
                LimitWindow {
                    label: if provider == Provider::Cursor {
                        "Monthly"
                    } else {
                        "5-hour session"
                    }
                    .into(),
                    used_percent: [28.0, 67.0, 41.0][provider.index()],
                    resets_at: Some(now + Duration::minutes([142, 84, 15040][provider.index()])),
                    duration_minutes: Some(if provider == Provider::Cursor {
                        43200
                    } else {
                        300
                    }),
                },
                LimitWindow {
                    label: if provider == Provider::Cursor {
                        "API usage"
                    } else {
                        "Weekly"
                    }
                    .into(),
                    used_percent: [46.0, 36.0, 62.0][provider.index()],
                    resets_at: Some(now + Duration::hours([79, 112, 250][provider.index()])),
                    duration_minutes: Some(10080),
                },
            ],
            checked_at: Some(now),
            stale: false,
            connected: true,
            sources: 1,
        })
        .collect();
    Snapshot {
        records,
        providers,
        refreshed_at: Some(now),
        notices: vec![],
    }
}
