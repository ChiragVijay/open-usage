use open_usage::{
    analytics::{self, ModelRow, Totals},
    model::UsageRecord,
};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Models,
    Activity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Model,
    Account,
    Tokens,
    Input,
    Output,
    CacheRead,
    CacheWrite,
    Sessions,
    Cost,
    Time,
    Project,
}
impl Column {
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Account => "Account",
            Self::Tokens => "Tokens",
            Self::Input => "Input",
            Self::Output => "Output",
            Self::CacheRead => "Cache read",
            Self::CacheWrite => "Cache write",
            Self::Sessions => "Sessions",
            Self::Cost => "Cost",
            Self::Time => "Time",
            Self::Project => "Project",
        }
    }
    fn numeric(self) -> bool {
        !matches!(self, Self::Model | Self::Account | Self::Project)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}
impl Sort {
    pub fn new(column: Column) -> Self {
        Self {
            column,
            descending: column.numeric(),
        }
    }
    pub fn select(&mut self, column: Column) {
        if self.column == column {
            self.descending = !self.descending;
        } else {
            *self = Self::new(column);
        }
    }
    pub fn marker(self, column: Column) -> &'static str {
        if self.column != column {
            ""
        } else if self.descending {
            " ↓"
        } else {
            " ↑"
        }
    }
    fn direction(self, order: Ordering) -> Ordering {
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn cost(self, a: Option<f64>, b: Option<f64>) -> Ordering {
        // Missing amounts stay last in either direction, including reported zero.
        match (a, b) {
            (Some(a), Some(b)) => self.direction(a.total_cmp(&b)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
    pub fn models(self, rows: &mut [ModelRow]) {
        rows.sort_by(|a, b| {
            let order = if self.column == Column::Cost {
                self.cost(a.totals.spend(), b.totals.spend())
            } else {
                self.direction(match self.column {
                    Column::Model => a.model.to_lowercase().cmp(&b.model.to_lowercase()),
                    Column::Account => a.provider.name().cmp(b.provider.name()),
                    Column::Tokens => a.totals.tokens.total().cmp(&b.totals.tokens.total()),
                    Column::Input => a.totals.tokens.input.cmp(&b.totals.tokens.input),
                    Column::Output => a.totals.tokens.output.cmp(&b.totals.tokens.output),
                    Column::CacheRead => {
                        a.totals.tokens.cache_read.cmp(&b.totals.tokens.cache_read)
                    }
                    Column::CacheWrite => a
                        .totals
                        .tokens
                        .cache_write
                        .cmp(&b.totals.tokens.cache_write),
                    Column::Sessions => a.totals.sessions.cmp(&b.totals.sessions),
                    _ => Ordering::Equal,
                })
            };
            order
                .then_with(|| a.model.cmp(&b.model))
                .then_with(|| a.provider.cmp(&b.provider))
        });
    }
    pub fn activity(self, rows: &mut [&UsageRecord]) {
        rows.sort_by(|a, b| {
            let order = if self.column == Column::Cost {
                self.cost(
                    analytics::record_cost(a).map(|c| c.0),
                    analytics::record_cost(b).map(|c| c.0),
                )
            } else {
                self.direction(match self.column {
                    Column::Time => a.timestamp.cmp(&b.timestamp),
                    Column::Account => a.provider.name().cmp(b.provider.name()),
                    Column::Model => a.model.to_lowercase().cmp(&b.model.to_lowercase()),
                    Column::Project => a.project.to_lowercase().cmp(&b.project.to_lowercase()),
                    Column::Tokens => a.tokens.total().cmp(&b.tokens.total()),
                    _ => Ordering::Equal,
                })
            };
            order
                .then_with(|| b.timestamp.cmp(&a.timestamp))
                .then_with(|| a.id.cmp(&b.id))
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CostFilter {
    All,
    Reported,
    Estimated,
    Unpriced,
}
impl CostFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Reported, Self::Estimated, Self::Unpriced];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All cost sources",
            Self::Reported => "Reported cost",
            Self::Estimated => "Estimated cost",
            Self::Unpriced => "Unpriced usage",
        }
    }
    pub fn totals(self, totals: &Totals) -> bool {
        match self {
            Self::All => true,
            Self::Reported => totals.reported_rows > 0,
            Self::Estimated => totals.estimated_rows > 0,
            Self::Unpriced => totals.events > totals.reported_rows + totals.estimated_rows,
        }
    }
    pub fn record(self, record: &UsageRecord) -> bool {
        matches!(
            (self, analytics::record_cost(record)),
            (Self::All, _)
                | (Self::Reported, Some((_, false)))
                | (Self::Estimated, Some((_, true)))
                | (Self::Unpriced, None)
        )
    }
}

pub fn matches_query(query: &str, fields: &[&str]) -> bool {
    let haystack = fields.join(" ").to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use open_usage::model::{Provider, Tokens};
    fn row(model: &str, tokens: u64, cost: Option<f64>) -> ModelRow {
        ModelRow {
            model: model.into(),
            provider: Provider::Claude,
            totals: Totals {
                tokens: Tokens {
                    input: tokens,
                    ..Default::default()
                },
                reported: cost.unwrap_or(0.),
                reported_rows: usize::from(cost.is_some()),
                events: 1,
                ..Default::default()
            },
        }
    }
    #[test]
    fn numeric_sort_uses_raw_values_and_reverses() {
        let mut rows = vec![
            row("small", 900, None),
            row("large", 1200, None),
            row("middle", 1000, None),
        ];
        let mut sort = Sort::new(Column::Tokens);
        sort.models(&mut rows);
        assert_eq!(
            rows.iter().map(|r| r.model.as_str()).collect::<Vec<_>>(),
            ["large", "middle", "small"]
        );
        sort.select(Column::Tokens);
        sort.models(&mut rows);
        assert_eq!(rows[0].model, "small");
        sort.select(Column::Model);
        sort.models(&mut rows);
        assert_eq!(rows[0].model, "large");
        assert!(!sort.descending);
    }
    #[test]
    fn missing_cost_stays_last_and_zero_is_a_real_amount() {
        for descending in [true, false] {
            let mut rows = vec![
                row("missing", 1, None),
                row("zero", 1, Some(0.)),
                row("paid", 1, Some(2.)),
            ];
            Sort {
                column: Column::Cost,
                descending,
            }
            .models(&mut rows);
            assert_eq!(rows[2].model, "missing");
            assert_eq!(rows[0].model, if descending { "paid" } else { "zero" });
        }
    }
    #[test]
    fn query_and_sources_match_partial_and_mixed_usage() {
        assert!(matches_query("CLAUDE sonnet", &["claude", "Sonnet 4.6"]));
        assert!(!matches_query("claude opus", &["claude", "Sonnet 4.6"]));
        assert!(matches_query("  ", &[]));
        let mixed = Totals {
            reported_rows: 1,
            estimated_rows: 1,
            events: 3,
            ..Default::default()
        };
        assert!(CostFilter::Reported.totals(&mixed));
        assert!(CostFilter::Estimated.totals(&mixed));
        assert!(CostFilter::Unpriced.totals(&mixed));
    }
    #[test]
    fn activity_sorts_entire_dataset_before_pagination() {
        let records = [1000, 30, 900].map(|tokens| UsageRecord {
            id: tokens.to_string(),
            provider: Provider::Cursor,
            timestamp: Utc.timestamp_opt(1_700_000_000 + tokens as i64, 0).unwrap(),
            model: "unknown-model".into(),
            session: String::new(),
            project: String::new(),
            tokens: Tokens {
                input: tokens,
                ..Default::default()
            },
            reported_cost: Some(tokens as f64 / 10.),
            source: String::new(),
        });
        let mut rows = records.iter().collect::<Vec<_>>();
        Sort::new(Column::Tokens).activity(&mut rows);
        assert_eq!(
            rows.iter()
                .take(2)
                .map(|r| r.tokens.total())
                .collect::<Vec<_>>(),
            [1000, 900]
        );
        let mut sort = Sort::new(Column::Time);
        sort.select(Column::Time);
        sort.activity(&mut rows);
        assert_eq!(rows[0].tokens.total(), 30);
    }
}
