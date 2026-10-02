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
            "usage [--refresh] [--online] [--demo] [--json] [--days 1|7|30|90]\n\nAccount usage and cached subscription allowances. Costs are provider reported; model prices are never inferred. --online checks existing account logins for this invocation.\nNo prompts, conversations, or credentials are printed."
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
    let is_demo = args.contains(&"--demo".into());
    let snapshot = if is_demo {
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
        let accounts = snapshot.providers.iter().filter(|status| config.enabled[status.provider.index()] && (status.connected || analysis.providers[status.provider.index()].events > 0)).map(|status| {
            let totals = &analysis.providers[status.provider.index()];
            serde_json::json!({"account":status.provider,"plan":status.plan,"reported_usage_cost_usd":totals.spend(),"cost_reporting_events":totals.reported_rows,"tokens":totals.tokens,"sessions":totals.sessions,"status":status})
        }).collect::<Vec<_>>();
        let models = analysis.models.iter().map(|row| serde_json::json!({"model":row.model,"account":row.provider,"tokens":row.totals.tokens,"reported_usage_cost_usd":row.totals.spend()})).collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"schema":"open-usage.v2","demo":is_demo,"days":period.days(),"reported_usage_cost_usd":analysis.totals.spend(),"cost_reporting_events":analysis.totals.reported_rows,"tokens":analysis.totals.tokens,"sessions":analysis.totals.sessions,"accounts":accounts,"models":models,"refreshed_at":snapshot.refreshed_at,"notices":snapshot.notices})
            )?
        );
    } else {
        println!(
            "Open Usage · {}{}\nTokens: {} · Sessions: {}",
            period.label(),
            if is_demo { " · EXAMPLE DATA" } else { "" },
            analytics::compact(analysis.totals.tokens.total()),
            analysis.totals.sessions
        );
        for status in &snapshot.providers {
            let totals = &analysis.providers[status.provider.index()];
            if !config.enabled[status.provider.index()] || (!status.connected && totals.events == 0)
            {
                continue;
            }
            let spend = totals
                .spend()
                .map(|cost| format!("${cost:.2}"))
                .unwrap_or_else(|| "Not reported".into());
            println!(
                "{} · {} · {} tokens · Reported usage cost: {}",
                status.provider.account_name(),
                status.plan.as_deref().unwrap_or("Plan unavailable"),
                analytics::compact(totals.tokens.total()),
                spend
            );
        }
    }
    Ok(())
}
