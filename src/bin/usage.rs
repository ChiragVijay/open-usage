use anyhow::{Result, bail};
use open_usage::{
    analytics::{self, Period},
    config::Config,
    demo,
    model::Snapshot,
    providers,
    store::Store,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "usage [--refresh] [--online] [--demo] [--json] [--days 1|7|30|90]\n\nReads local history and cached account limits. --online enables provider account checks for this invocation.\nNo prompts, conversations, or credentials are printed."
        );
        return Ok(());
    }
    let mut config = Config::load()?;
    if args.contains(&"--online".into()) {
        config.online = true;
    }
    let days = args
        .iter()
        .position(|arg| arg == "--days")
        .map(|index| args.get(index + 1).map(String::as_str).unwrap_or(""))
        .unwrap_or("30");
    let period = match days {
        "1" => Period::Today,
        "7" => Period::Week,
        "30" => Period::Month,
        "90" => Period::Quarter,
        _ => bail!("--days must be 1, 7, 30, or 90"),
    };
    let snapshot = if args.contains(&"--demo".into()) {
        demo::snapshot()
    } else {
        let cached = Store::open()?.load()?.unwrap_or_else(Snapshot::default);
        if args.contains(&"--refresh".into())
            || args.contains(&"--online".into())
            || cached.refreshed_at.is_none()
        {
            providers::collect(&config, &cached)?
        } else {
            cached
        }
    };
    let analysis = analytics::analyze(&snapshot, &config, period, None, chrono::Utc::now());
    if args.contains(&"--json".into()) {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"schema":"open-usage.v1","demo":args.contains(&"--demo".into()),"days":period.days(),"estimated_api_equivalent_usd":analysis.totals.estimated,"reported_usd":analysis.totals.reported,"tokens":analysis.totals.tokens,"sessions":analysis.totals.sessions,"unpriced_events":analysis.totals.unpriced_rows,"models":analysis.models.iter().map(|row|serde_json::json!({"model":row.model,"provider":row.provider,"tokens":row.totals.tokens,"estimated_usd":row.totals.estimated,"reported_usd":row.totals.reported,"unpriced_events":row.totals.unpriced_rows})).collect::<Vec<_>>(),"providers":snapshot.providers,"refreshed_at":snapshot.refreshed_at,"notices":snapshot.notices})
            )?
        );
    } else {
        println!(
            "Open Usage · {}{}\nEstimated API equivalent: ${:.2}\nReported spend: ${:.2}\nTokens: {} · Sessions: {} · Unpriced events: {}",
            period.label(),
            if args.contains(&"--demo".into()) {
                " · EXAMPLE DATA"
            } else {
                ""
            },
            analysis.totals.estimated,
            analysis.totals.reported,
            analytics::compact(analysis.totals.tokens.total()),
            analysis.totals.sessions,
            analysis.totals.unpriced_rows
        );
        for status in &snapshot.providers {
            println!("{}: {}", status.provider.name(), status.detail);
        }
    }
    Ok(())
}
