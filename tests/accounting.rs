use chrono::{Duration, Utc};
use open_usage::{
    analytics::{self, Period},
    config::Config,
    model::*,
    providers::{history, live},
    store::Store,
};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn fixture(text: &str, extension: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(format!("session.{extension}"));
    fs::write(&path, text).unwrap();
    (dir, path)
}
fn record(model: &str) -> UsageRecord {
    UsageRecord {
        id: "a".into(),
        provider: Provider::Codex,
        timestamp: Utc::now() - Duration::minutes(1),
        model: model.into(),
        session: "session".into(),
        project: "sample".into(),
        tokens: Tokens {
            input: 100,
            output: 50,
            cache_read: 200,
            cache_write: 0,
        },
        reported_cost: None,
        source: "fixture".into(),
    }
}
fn token_line(total: u64, input: u64, cache: u64, output: u64) -> String {
    json!({"timestamp":"2026-10-02T13:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":total,"input_tokens":input,"cached_input_tokens":cache,"output_tokens":output,"reasoning_output_tokens":10},"last_token_usage":{"input_tokens":input,"cached_input_tokens":cache,"output_tokens":output}}}}).to_string()
}

#[test]
fn codex_cumulative_counts_and_repeated_snapshots_do_not_double_count() {
    let lines = [
        json!({"type":"session_meta","payload":{"id":"session-1","cwd":"C:\\dev\\atlas"}})
            .to_string(),
        json!({"type":"turn_context","payload":{"model":"gpt-5.4"}}).to_string(),
        token_line(150, 100, 40, 50),
        token_line(150, 100, 40, 50),
        token_line(220, 150, 60, 70),
    ]
    .join("\n");
    let (_dir, path) = fixture(&lines, "jsonl");
    let parsed = history::read_jsonl(&path, Provider::Codex).unwrap();
    assert_eq!(parsed.records.len(), 2);
    assert_eq!(parsed.skipped, 0);
    let mut tokens = Tokens::default();
    for record in parsed.records {
        assert_eq!(record.project, "atlas");
        assert_eq!(record.model, "gpt-5.4");
        tokens.add(&record.tokens);
    }
    assert_eq!(
        tokens,
        Tokens {
            input: 90,
            cache_read: 60,
            output: 70,
            cache_write: 0
        }
    );
    assert_eq!(tokens.total(), 220);
}
#[test]
fn codex_counter_reset_preserves_new_usage() {
    let (_dir, path) = fixture(
        &[token_line(150, 100, 40, 50), token_line(20, 15, 5, 5)].join("\n"),
        "jsonl",
    );
    let parsed = history::read_jsonl(&path, Provider::Codex).unwrap();
    assert_eq!(
        parsed.records.iter().map(|r| r.tokens.total()).sum::<u64>(),
        170
    );
}
#[test]
fn incomplete_json_does_not_discard_completed_usage() {
    let (_dir, path) = fixture(
        &format!("{}\n{{\"timestamp\":", token_line(150, 100, 40, 50)),
        "jsonl",
    );
    let parsed = history::read_jsonl(&path, Provider::Codex).unwrap();
    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.skipped, 1);
}
#[test]
fn claude_streaming_blocks_count_once_and_subagents_are_included() {
    let line = |id: &str, out, sidechain| {
        json!({"type":"assistant","sessionId":"one","requestId":id,"isSidechain":sidechain,"timestamp":"2026-10-02T13:00:00Z","message":{"id":id,"model":"claude-sonnet-4-6","usage":{"input_tokens":100,"output_tokens":out,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}}}).to_string()
    };
    let (_dir, path) = fixture(
        &[
            line("msg-1", 10, false),
            line("msg-1", 30, false),
            line("msg-sub", 20, true),
        ]
        .join("\n"),
        "jsonl",
    );
    let parsed = history::read_jsonl(&path, Provider::Claude).unwrap();
    assert_eq!(parsed.records.len(), 2);
    assert_eq!(
        parsed.records.iter().map(|r| r.tokens.output).sum::<u64>(),
        50
    );
    assert_eq!(
        parsed.records.iter().map(|r| r.tokens.total()).sum::<u64>(),
        750
    );
}
#[test]
fn cursor_csv_handles_bom_quoted_fields_and_repeated_billed_rows() {
    let text = "\u{feff}Date,Model,Input Tokens,Output Tokens,Cache Read Tokens,Cache Write Tokens,Cost\n2026-10-02T13:00:00Z,\"model, special\",10,5,20,2,$0.50\n2026-10-02T13:00:00Z,\"model, special\",10,5,20,2,$0.50\n";
    let (_dir, path) = fixture(text, "csv");
    let records = history::read_cursor_csv(&path).unwrap();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].id, records[1].id);
    assert_eq!(records[0].tokens.total(), 37);
    assert_eq!(records[0].reported_cost, Some(0.5));
}
#[test]
fn cursor_csv_rejects_negative_costs_and_unknown_structure() {
    let (_dir, path) = fixture("Date,Model,Cost\n2026-10-02T13:00:00Z,x,-2\n", "csv");
    assert!(history::read_cursor_csv(&path).is_err());
    fs::write(&path, "foo,bar\n1,2\n").unwrap();
    assert!(history::read_cursor_csv(&path).is_err());
}
#[test]
fn model_names_never_invent_account_cost() {
    let snapshot = Snapshot {
        records: vec![record("unannounced-model")],
        ..Default::default()
    };
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        None,
        Utc::now(),
    );
    assert_eq!(analysis.totals.tokens.total(), 350);
    assert_eq!(analysis.totals.spend(), None);
    assert_eq!(
        analysis
            .buckets
            .iter()
            .map(|bucket| bucket.cost.iter().sum::<f64>())
            .sum::<f64>(),
        0.0
    );
}
#[test]
fn account_reported_cost_is_the_only_cost() {
    let mut event = record("gpt-5.4");
    event.reported_cost = Some(3.25);
    let snapshot = Snapshot {
        records: vec![event],
        ..Default::default()
    };
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        None,
        Utc::now(),
    );
    assert_eq!(analysis.totals.reported, 3.25);
    assert_eq!(analysis.totals.spend(), Some(3.25));
    assert_eq!(
        analysis
            .buckets
            .iter()
            .map(|bucket| bucket.cost.iter().sum::<f64>())
            .sum::<f64>(),
        3.25
    );
}
#[test]
fn reported_zero_is_distinct_from_missing_cost() {
    let mut event = record("custom");
    event.reported_cost = Some(0.0);
    let snapshot = Snapshot {
        records: vec![event],
        ..Default::default()
    };
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        None,
        Utc::now(),
    );
    assert_eq!(analysis.totals.spend(), Some(0.0));
}
#[test]
fn provider_and_time_filters_apply_to_every_total() {
    let now = Utc::now();
    let mut snapshot = Snapshot::default();
    let mut old = record("gpt-5.4");
    old.timestamp = now - Duration::days(120);
    let mut claude = record("claude-sonnet-4-6");
    claude.provider = Provider::Claude;
    snapshot.records = vec![record("gpt-5.4"), old, claude];
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Month,
        Some(Provider::Codex),
        now,
    );
    assert_eq!(analysis.totals.events, 1);
    assert_eq!(analysis.totals.sessions, 1);
    assert_eq!(analysis.totals.tokens.total(), 350);
    assert_eq!(
        analysis
            .buckets
            .iter()
            .map(|b| b.tokens.iter().sum::<u64>())
            .sum::<u64>(),
        350
    );
}
#[test]
fn codex_multibucket_prefers_main_allowance_and_real_duration() {
    let limits = live::codex_limits(
        &json!({"rateLimits":{"primary":{"usedPercent":99}},"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25,"windowDurationMins":43200,"resetsAt":1791000000}}}}),
    );
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].label, "Monthly");
    assert_eq!(limits[0].remaining(), 75.0);
    assert!(limits[0].resets_at.is_some());
}
#[test]
fn provider_quota_percentages_and_epoch_units_are_normalized() {
    let codex = live::codex_limits(
        &json!({"rate_limit":{"primary_window":{"used_percent":30,"limit_window_seconds":18000,"reset_at":1791000000}}}),
    );
    assert_eq!(codex[0].duration_minutes, Some(300));
    let claude = live::claude_limits(
        &json!({"five_hour":{"utilization":67,"resets_at":"2026-10-03T00:00:00Z"}}),
    );
    assert_eq!(claude[0].remaining(), 33.0);
    let cursor = live::cursor_limits(
        &json!({"billingCycleEnd":"1791000000000","planUsage":{"totalPercentUsed":120,"apiPercentUsed":25}}),
    );
    assert_eq!(cursor[0].remaining(), 0.0);
    assert_eq!(cursor[1].remaining(), 75.0);
    assert!(cursor[0].resets_at.is_some());
}
#[test]
fn cursor_api_preserves_identical_events_and_converts_cents() {
    let event = json!({"timestamp":"1790946000000","model":"gpt-5.4","tokenUsage":{"inputTokens":10,"outputTokens":5,"cacheReadTokens":20,"cacheWriteTokens":2,"totalCents":50}});
    let records = live::parse_cursor_events(&[event.clone(), event], "account-hash").unwrap();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].id, records[1].id);
    assert_eq!(records[0].reported_cost, Some(0.5));
    assert_eq!(records[0].tokens.total(), 37);
}
#[test]
fn sqlite_roundtrip_and_file_cache_invalidate_on_change() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("usage.db");
    let store = Store::at(&path).unwrap();
    let snapshot = Snapshot {
        records: vec![record("gpt-5.4")],
        ..Default::default()
    };
    store.save(&snapshot).unwrap();
    assert_eq!(store.load().unwrap().unwrap().records.len(), 1);
    store
        .cache(&dir.path().join("source"), 42, 100, &snapshot.records)
        .unwrap();
    assert_eq!(
        store
            .cached(&dir.path().join("source"), 42, 100)
            .unwrap()
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .cached(&dir.path().join("source"), 43, 100)
            .unwrap()
            .is_none()
    );
    let stored = fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&stored).contains("access_token"));
}
#[test]
fn export_respects_account_filters_and_leaves_missing_cost_blank() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("export.csv");
    let snapshot = Snapshot {
        records: vec![record("unknown")],
        ..Default::default()
    };
    let count = analytics::export_csv(
        &path,
        &snapshot,
        &Config::default(),
        Period::Today,
        Some(Provider::Codex),
    )
    .unwrap();
    assert_eq!(count, 1);
    let mut csv = csv::Reader::from_path(path).unwrap();
    assert_eq!(&csv.headers().unwrap()[1], "account");
    assert!(
        !csv.headers()
            .unwrap()
            .iter()
            .any(|header| header.contains("estimated"))
    );
    let row = csv.records().next().unwrap().unwrap();
    assert_eq!(&row[9], "");
}
#[test]
fn claude_models_used_in_cursor_belong_only_to_cursor() {
    let mut cursor = record("claude-sonnet-4-6");
    cursor.provider = Provider::Cursor;
    cursor.reported_cost = Some(1.25);
    let mut claude = record("claude-sonnet-4-6");
    claude.provider = Provider::Claude;
    let snapshot = Snapshot {
        records: vec![cursor, claude],
        ..Default::default()
    };
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        None,
        Utc::now(),
    );
    assert_eq!(
        analysis.providers[Provider::Cursor.index()].spend(),
        Some(1.25)
    );
    assert_eq!(analysis.providers[Provider::Claude.index()].spend(), None);
    assert_eq!(analysis.models.len(), 2);
    let cursor_only = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        Some(Provider::Cursor),
        Utc::now(),
    );
    assert_eq!(cursor_only.models.len(), 1);
    assert_eq!(cursor_only.models[0].provider, Provider::Cursor);
    assert_eq!(cursor_only.totals.tokens.total(), 350);
}

#[test]
fn invalid_reported_costs_do_not_pollute_totals() {
    let mut invalid = record("unknown");
    invalid.reported_cost = Some(f64::NAN);
    let mut negative = record("another");
    negative.reported_cost = Some(-2.0);
    let snapshot = Snapshot {
        records: vec![invalid, negative],
        ..Default::default()
    };
    let analysis = analytics::analyze(
        &snapshot,
        &Config::default(),
        Period::Today,
        None,
        Utc::now(),
    );
    assert_eq!(analysis.totals.spend(), None);
    assert_eq!(analysis.totals.tokens.total(), 700);
}

#[test]
fn legacy_price_overrides_cannot_create_account_cost() {
    let config: Config = serde_json::from_value(json!({"prices":{"custom":{"input":999,"output":999,"cache_read":999,"cache_write":999}},"monthly_budget":100})).unwrap();
    let snapshot = Snapshot {
        records: vec![record("custom")],
        ..Default::default()
    };
    let analysis = analytics::analyze(&snapshot, &config, Period::Today, None, Utc::now());
    assert_eq!(analysis.totals.spend(), None);
    assert_eq!(analysis.totals.tokens.total(), 350);
    assert!(
        serde_json::to_value(config)
            .unwrap()
            .get("prices")
            .is_none()
    );
}
