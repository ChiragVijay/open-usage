pub mod discovery;
pub mod history;
pub mod live;

use crate::{
    config::Config,
    model::{Provider, Snapshot, UsageRecord},
    store::Store,
};
use anyhow::Result;
use chrono::Utc;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;

pub fn homes(provider: Provider, config: &Config) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let default = match provider {
        Provider::Codex => std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex")),
        Provider::Claude => std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude")),
        Provider::Cursor => {
            if cfg!(target_os = "windows") {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join("AppData/Roaming"))
                    .join("Cursor")
            } else {
                home.join(".cursor")
            }
        }
    };
    let mut paths = vec![default];
    if provider == Provider::Cursor {
        paths.push(home.join(".cursor"));
        if cfg!(target_os = "macos") {
            paths.push(home.join("Library/Application Support/Cursor"));
        }
    }
    paths.extend(config.custom_homes[provider.index()].iter().cloned());
    paths = paths
        .into_iter()
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    paths.retain(|path| seen.insert(path.clone()));
    paths
}

struct LocalScan {
    installations: Vec<String>,
    detected: bool,
    sources: usize,
    records: BTreeMap<String, UsageRecord>,
    notices: Vec<String>,
}

pub fn collect(config: &Config, previous: &Snapshot) -> Result<Snapshot> {
    collect_with(config, previous, |_| {})
}

/// Scans every account at once. Allowance checks are published as soon as they
/// return, without waiting for the other accounts or for local history.
pub fn collect_with(
    config: &Config,
    previous: &Snapshot,
    progress: impl Fn(&Snapshot),
) -> Result<Snapshot> {
    crate::pricing::start();
    let store = Arc::new(Mutex::new(Store::open()?));
    let environment = discovery::Environment::current();
    let mut snapshot = previous.clone();
    snapshot.notices.clear();
    for provider in Provider::ALL {
        let status = &mut snapshot.providers[provider.index()];
        if !config.enabled[provider.index()] {
            status.detail = "Paused in Connections".into();
            snapshot
                .records
                .retain(|record| record.provider != provider);
        }
    }

    enum Event {
        Accounts(Provider, Result<live::LiveData, String>),
        History(Provider, LocalScan),
    }
    let (sender, receiver) = mpsc::channel();
    let mut pending = 0;
    for provider in Provider::ALL {
        if !config.enabled[provider.index()] {
            continue;
        }
        let directories = homes(provider, config);
        let check_account = config.online
            && (snapshot.providers[provider.index()].detected
                || snapshot.providers[provider.index()].connected
                || snapshot.providers[provider.index()].checked_at.is_some()
                || !discovery::detect(provider, &directories, &environment).is_empty());
        if check_account {
            pending += 1;
            let sender = sender.clone();
            let directories = directories.clone();
            let history = provider == Provider::Cursor && config.cursor_imports.is_empty();
            thread::spawn(move || {
                let result =
                    live::fetch(provider, &directories, history).map_err(|e| e.to_string());
                let _ = sender.send(Event::Accounts(provider, result));
            });
        }
        pending += 1;
        let sender = sender.clone();
        let config = config.clone();
        let previous_records = snapshot
            .records
            .iter()
            .filter(|record| record.provider == provider)
            .cloned()
            .collect::<Vec<_>>();
        let store = store.clone();
        thread::spawn(move || {
            let environment = discovery::Environment::current();
            let scan = scan_local(
                provider,
                &config,
                &directories,
                &environment,
                &store,
                &previous_records,
            );
            let _ = sender.send(Event::History(provider, scan));
        });
    }
    drop(sender);

    let mut live_error = [None, None, None];
    let mut history_done = [false; 3];
    while pending > 0 {
        let Ok(event) = receiver.recv() else {
            break;
        };
        pending -= 1;
        match event {
            Event::Accounts(provider, result) => {
                let status = &mut snapshot.providers[provider.index()];
                match result {
                    Ok(data) => {
                        status.limits = data.limits;
                        status.plan = data.plan;
                        status.checked_at = Some(Utc::now());
                        status.stale = false;
                        status.connected = true;
                        live_error[provider.index()] = None;
                        if provider == Provider::Cursor && config.cursor_imports.is_empty() {
                            snapshot
                                .records
                                .retain(|record| record.provider != provider);
                            snapshot.records.extend(data.records);
                        }
                    }
                    Err(error) => {
                        live_error[provider.index()] = Some(error);
                        status.stale = true;
                    }
                }
            }
            Event::History(provider, scan) => {
                let status = &mut snapshot.providers[provider.index()];
                status.installations = scan.installations;
                status.detected = scan.detected;
                status.sources = scan.sources;
                if scan.sources > 0 {
                    status.connected = true;
                }
                // Cursor account history arrives with the allowance check. A local
                // scan must not wipe it when no CSV import is selected.
                let cursor_api = provider == Provider::Cursor
                    && config.cursor_imports.is_empty()
                    && config.online;
                if !cursor_api {
                    snapshot
                        .records
                        .retain(|record| record.provider != provider);
                    snapshot.records.extend(scan.records.into_values());
                }
                snapshot.notices.extend(scan.notices);
                history_done[provider.index()] = true;
            }
        }
        for provider in Provider::ALL {
            if config.enabled[provider.index()] {
                describe(
                    &mut snapshot.providers[provider.index()],
                    live_error[provider.index()].as_deref(),
                    history_done[provider.index()],
                    config.online,
                );
            }
        }
        snapshot
            .records
            .sort_by_key(|record| std::cmp::Reverse(record.timestamp));
        snapshot.refreshed_at = Some(Utc::now());
        progress(&snapshot);
    }
    store
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .save(&snapshot)?;
    Ok(snapshot)
}

fn describe(
    status: &mut crate::model::ProviderStatus,
    error: Option<&str>,
    history: bool,
    online: bool,
) {
    if let Some(error) = error
        && (status.checked_at.is_none() || status.stale)
    {
        status.detail = error.to_owned();
        return;
    }
    if !status.stale && status.checked_at.is_some() {
        status.detail = if status.provider == Provider::Cursor {
            "Account history · across your machines".into()
        } else if status.sources > 0 || !history {
            "Local history + live account limits".into()
        } else {
            "Live account limits".into()
        };
        return;
    }
    if !history {
        return;
    }
    status.detail = if status.sources > 0 {
        if status.provider == Provider::Cursor && !online {
            "Cached Cursor account history".into()
        } else {
            format!("{} history files · local", status.sources)
        }
    } else if status.detected {
        "Detected on this device · no local usage yet".into()
    } else if status.provider == Provider::Cursor {
        "Import a usage CSV or enable account checks".into()
    } else {
        format!(
            "Use {} to create local history, or add its data folder",
            status.provider.name()
        )
    };
}

fn scan_local(
    provider: Provider,
    config: &Config,
    directories: &[PathBuf],
    environment: &discovery::Environment,
    store: &Mutex<Store>,
    previous: &[UsageRecord],
) -> LocalScan {
    let mut scan = LocalScan {
        installations: discovery::detect(provider, directories, environment),
        detected: false,
        sources: 0,
        records: BTreeMap::new(),
        notices: vec![],
    };
    scan.detected = !scan.installations.is_empty();
    let mut file_errors = 0;
    if provider != Provider::Cursor {
        let subdirs: &[&str] = if provider == Provider::Codex {
            &["sessions", "archived_sessions"]
        } else {
            &["projects"]
        };
        for home in directories {
            for subdir in subdirs {
                let root = home.join(subdir);
                if !root.exists() {
                    continue;
                }
                for entry in WalkDir::new(root).follow_links(false).into_iter() {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => {
                            file_errors += 1;
                            continue;
                        }
                    };
                    if !entry.file_type().is_file()
                        || entry.path().extension().is_none_or(|ext| ext != "jsonl")
                    {
                        continue;
                    }
                    scan.sources += 1;
                    let parsed = (|| -> Result<Vec<UsageRecord>> {
                        let metadata = entry.metadata()?;
                        let modified = metadata
                            .modified()?
                            .duration_since(UNIX_EPOCH)?
                            .as_nanos()
                            .min(i64::MAX as u128) as u64;
                        let cached = store
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner())
                            .cached(entry.path(), metadata.len(), modified)?;
                        if let Some(cached) = cached {
                            return Ok(cached);
                        }
                        let parsed = history::read_jsonl(entry.path(), provider)?;
                        if parsed.skipped == 0 {
                            store
                                .lock()
                                .unwrap_or_else(|poison| poison.into_inner())
                                .cache(entry.path(), metadata.len(), modified, &parsed.records)?;
                        } else {
                            file_errors += parsed.skipped;
                        }
                        Ok(parsed.records)
                    })();
                    match parsed {
                        Ok(records) => {
                            for record in records {
                                scan.records.insert(record.id.clone(), record);
                            }
                        }
                        Err(_) => file_errors += 1,
                    }
                }
            }
        }
    } else {
        for path in &config.cursor_imports {
            match history::read_cursor_csv(path) {
                Ok(records) => {
                    scan.sources += 1;
                    for record in records {
                        scan.records.insert(record.id.clone(), record);
                    }
                }
                Err(_) => file_errors += 1,
            }
        }
        if config.cursor_imports.is_empty() && !config.online {
            for record in previous {
                scan.records.insert(record.id.clone(), record.clone());
                scan.sources = scan.sources.max(1);
            }
        }
    }
    if file_errors > 0 {
        scan.notices.push(format!(
            "{}: {file_errors} unreadable files or incomplete rows. Refresh to retry.",
            provider.name()
        ));
    }
    scan
}
