pub mod history;
pub mod live;

use crate::{
    config::Config,
    model::{Provider, Snapshot, UsageRecord},
    store::Store,
};
use anyhow::Result;
use chrono::Utc;
use std::{collections::BTreeMap, path::PathBuf, time::UNIX_EPOCH};
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
    paths.extend(config.custom_homes[provider.index()].iter().cloned());
    paths = paths
        .into_iter()
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    paths.retain(|path| seen.insert(path.clone()));
    paths
}

pub fn collect(config: &Config, previous: &Snapshot) -> Result<Snapshot> {
    let store = Store::open()?;
    let mut snapshot = Snapshot::default();
    let mut records = BTreeMap::<String, UsageRecord>::new();
    for provider in Provider::ALL {
        let status = &mut snapshot.providers[provider.index()];
        if !config.enabled[provider.index()] {
            status.detail = "Paused in Connections".into();
            continue;
        }
        if let Some(old) = previous.providers.iter().find(|p| p.provider == provider) {
            status.limits = old.limits.clone();
            status.checked_at = old.checked_at;
            status.plan = old.plan.clone();
            status.stale = true;
        }
        let directories = homes(provider, config);
        let mut file_errors = 0;
        if provider != Provider::Cursor {
            let subdirs: &[&str] = if provider == Provider::Codex {
                &["sessions", "archived_sessions"]
            } else {
                &["projects"]
            };
            for home in &directories {
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
                        status.sources += 1;
                        let result = (|| -> Result<Vec<UsageRecord>> {
                            let metadata = entry.metadata()?;
                            let modified = metadata
                                .modified()?
                                .duration_since(UNIX_EPOCH)?
                                .as_nanos()
                                .min(i64::MAX as u128)
                                as u64;
                            if let Some(cached) =
                                store.cached(entry.path(), metadata.len(), modified)?
                            {
                                return Ok(cached);
                            }
                            let parsed = history::read_jsonl(entry.path(), provider)?;
                            // Invalid/truncated rows must be retried; do not cache them as complete.
                            if parsed.skipped == 0 {
                                store.cache(
                                    entry.path(),
                                    metadata.len(),
                                    modified,
                                    &parsed.records,
                                )?;
                            } else {
                                file_errors += parsed.skipped;
                            }
                            Ok(parsed.records)
                        })();
                        match result {
                            Ok(parsed) => {
                                for record in parsed {
                                    records.insert(record.id.clone(), record);
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
                    Ok(parsed) => {
                        status.sources += 1;
                        for record in parsed {
                            records.insert(record.id.clone(), record);
                        }
                    }
                    Err(_) => {
                        file_errors += 1;
                    }
                }
            }
        }
        status.connected = status.sources > 0;
        status.detail = if status.connected {
            format!("{} history files · local", status.sources)
        } else if provider == Provider::Cursor {
            "Import a usage CSV or enable account checks".into()
        } else {
            format!(
                "Use {} to create local history, or add its data folder",
                provider.name()
            )
        };
        if file_errors > 0 {
            snapshot.notices.push(format!(
                "{}: {file_errors} unreadable files or incomplete rows. Refresh to retry.",
                provider.name()
            ));
        }
        if config.online {
            match live::fetch(provider, &directories) {
                Ok(data) => {
                    status.limits = data.limits;
                    status.plan = data.plan;
                    status.checked_at = Some(Utc::now());
                    status.stale = false;
                    status.connected = true;
                    status.detail = if provider == Provider::Cursor {
                        "Account history · across your machines".into()
                    } else {
                        "Local history + live account limits".into()
                    };
                    if provider == Provider::Cursor && config.cursor_imports.is_empty() {
                        for record in data.records {
                            records.insert(record.id.clone(), record);
                        }
                    }
                }
                Err(error) => {
                    status.detail = error.to_string();
                    if provider == Provider::Cursor && config.cursor_imports.is_empty() {
                        for record in previous.records.iter().filter(|r| r.provider == provider) {
                            records.insert(record.id.clone(), record.clone());
                        }
                    }
                }
            }
        }
    }
    snapshot.records = records.into_values().collect();
    snapshot
        .records
        .sort_by_key(|record| std::cmp::Reverse(record.timestamp));
    snapshot.refreshed_at = Some(Utc::now());
    store.save(&snapshot)?;
    Ok(snapshot)
}
