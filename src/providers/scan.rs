//! Local history collection: parser adapters share cache and recovery policy here.
use super::{antigravity, discovery, history, homes, kiro, local, opencode};
use crate::{
    config::Config,
    model::{Provider, UsageRecord},
    store::Store,
};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Mutex,
};
use walkdir::WalkDir;

pub(super) struct LocalScan {
    pub installations: Vec<String>,
    pub detected: bool,
    pub sources: usize,
    pub records: BTreeMap<String, UsageRecord>,
    pub notices: Vec<String>,
    pub cached_limits: Option<kiro::CachedLimits>,
    errors: usize,
    complete_sources: BTreeSet<String>,
}

impl LocalScan {
    fn extend(&mut self, records: impl IntoIterator<Item = UsageRecord>) {
        for record in records {
            self.records.insert(record.id.clone(), record);
        }
    }

    fn preserve(&mut self, records: impl IntoIterator<Item = UsageRecord>) {
        for record in records {
            if !self.complete_sources.contains(&record.source) {
                self.records.entry(record.id.clone()).or_insert(record);
            }
        }
    }

    /// Fresh records win; a failed or incomplete source retains its older usage.
    fn accept(&mut self, path: &Path, result: Result<history::Parsed>, previous: &[UsageRecord]) {
        let errors = match result {
            Ok(parsed) => {
                if parsed.skipped == 0 {
                    let source = path.to_string_lossy().into_owned();
                    self.records.retain(|_, record| record.source != source);
                    self.complete_sources.insert(source);
                }
                self.extend(parsed.records);
                parsed.skipped
            }
            Err(_) => 1,
        };
        self.errors += errors;
        if errors > 0 {
            self.preserve(
                previous
                    .iter()
                    .filter(|record| record.source == path.to_string_lossy())
                    .cloned(),
            );
        }
    }

    // Discovery/traversal failures have no reliable source path to recover by.
    fn discovery_failed(&mut self, previous: &[UsageRecord]) {
        self.errors += 1;
        self.preserve(previous.iter().cloned());
    }
}

fn complete(records: Vec<UsageRecord>) -> history::Parsed {
    history::Parsed {
        records,
        skipped: 0,
    }
}

/// Cache only complete parses of sources that stayed unchanged throughout the read.
/// The lock covers storage access, never parsing an agent-owned source.
fn read_cached(
    path: &Path,
    store: &Mutex<Store>,
    parse: impl FnOnce() -> Result<history::Parsed>,
) -> Result<history::Parsed> {
    let (size, modified) = local::fingerprint(path)?;
    if let Some(records) = store
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .cached(path, size, modified)?
    {
        return Ok(complete(records));
    }
    let parsed = parse()?;
    if parsed.skipped == 0 && local::fingerprint(path).is_ok_and(|after| after == (size, modified))
    {
        store
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .cache(path, size, modified, &parsed.records)?;
    }
    Ok(parsed)
}

pub(super) fn scan_local(
    provider: Provider,
    config: &Config,
    directories: &[PathBuf],
    environment: &discovery::Environment,
    store: &Mutex<Store>,
    previous: &[UsageRecord],
) -> LocalScan {
    let installations = discovery::detect(provider, directories, environment);
    let mut scan = LocalScan {
        detected: !installations.is_empty(),
        installations,
        sources: 0,
        records: BTreeMap::new(),
        notices: vec![],
        cached_limits: None,
        errors: 0,
        complete_sources: BTreeSet::new(),
    };
    match provider {
        Provider::Kiro => match kiro::cached_limits(directories) {
            Ok(cached) => scan.cached_limits = cached,
            Err(_) => scan.discovery_failed(previous),
        },
        Provider::Antigravity | Provider::OpenCode => {
            let paths = if provider == Provider::OpenCode {
                opencode::databases(directories)
            } else {
                antigravity::databases(directories)
            };
            match paths {
                Ok(paths) => {
                    for path in paths {
                        scan.sources += 1;
                        let result = read_cached(&path, store, || {
                            if provider == Provider::OpenCode {
                                opencode::history(&path).map(complete)
                            } else {
                                antigravity::history_with_warnings(&path)
                                    .map(|(records, skipped)| history::Parsed { records, skipped })
                            }
                        });
                        scan.accept(&path, result, previous);
                    }
                }
                Err(_) => scan.discovery_failed(previous),
            }
        }
        Provider::Codex | Provider::Claude => {
            if provider == Provider::Codex {
                match opencode::databases(&homes(Provider::OpenCode, config)) {
                    Ok(paths) => {
                        for path in paths {
                            // This is a different projection of the OpenCode database;
                            // it cannot share its path-only cache with OpenCode history.
                            let result = opencode::codex_history(&path).map(complete);
                            if result
                                .as_ref()
                                .is_ok_and(|parsed| !parsed.records.is_empty())
                            {
                                scan.sources += 1;
                                scan.detected = true;
                            }
                            scan.accept(&path, result, previous);
                        }
                    }
                    Err(_) => scan.discovery_failed(previous),
                }
            }
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
                    for entry in WalkDir::new(root).follow_links(false) {
                        let entry = match entry {
                            Ok(entry) => entry,
                            Err(_) => {
                                scan.discovery_failed(previous);
                                continue;
                            }
                        };
                        if !entry.file_type().is_file()
                            || entry.path().extension().is_none_or(|ext| ext != "jsonl")
                        {
                            continue;
                        }
                        scan.sources += 1;
                        let result = read_cached(entry.path(), store, || {
                            history::read_jsonl(entry.path(), provider)
                        });
                        scan.accept(entry.path(), result, previous);
                    }
                }
            }
        }
        Provider::Cursor => {
            for path in &config.cursor_imports {
                let result = history::read_cursor_csv(path).map(complete);
                if result.is_ok() {
                    scan.sources += 1;
                }
                scan.accept(path, result, previous);
            }
            if config.cursor_imports.is_empty() && !config.online {
                scan.extend(previous.iter().cloned());
                if !previous.is_empty() {
                    scan.sources = 1;
                }
            }
        }
    }
    if scan.errors > 0 {
        scan.notices.push(format!(
            "{}: {} unreadable files or incomplete rows. Refresh to retry.",
            provider.name(),
            scan.errors
        ));
    }
    scan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Tokens;
    use std::fs;
    use tempfile::TempDir;

    fn record(path: &Path, id: &str, provider: Provider, input: u64) -> UsageRecord {
        UsageRecord {
            id: id.into(),
            provider,
            timestamp: chrono::Utc::now(),
            model: "fixture-model".into(),
            session: "fixture-session".into(),
            project: String::new(),
            tokens: Tokens {
                input,
                ..Tokens::default()
            },
            reported_cost: None,
            source: path.to_string_lossy().into_owned(),
        }
    }

    fn scan(
        dir: &Path,
        provider: Provider,
        config: &Config,
        previous: &[UsageRecord],
    ) -> LocalScan {
        let environment = discovery::Environment {
            home: dir.into(),
            path: vec![],
            local_data: dir.into(),
            program_dirs: vec![],
            applications: vec![],
        };
        let store = Mutex::new(Store::at(&dir.join("cache.db")).unwrap());
        scan_local(
            provider,
            config,
            &[dir.into()],
            &environment,
            &store,
            previous,
        )
    }

    #[test]
    fn stable_cache_hits_skip_parsing_and_changed_sources_are_reparsed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history.jsonl");
        fs::write(&path, b"first").unwrap();
        let store = Mutex::new(Store::at(&dir.path().join("cache.db")).unwrap());
        read_cached(&path, &store, || {
            Ok(complete(vec![record(&path, "old", Provider::Claude, 10)]))
        })
        .unwrap();
        let cached = read_cached(&path, &store, || {
            panic!("A stable source should use its cache")
        })
        .unwrap();
        assert_eq!(cached.records[0].id, "old");
        fs::write(&path, b"a changed source").unwrap();
        let changed = read_cached(&path, &store, || {
            Ok(complete(vec![record(&path, "new", Provider::Claude, 20)]))
        })
        .unwrap();
        assert_eq!(changed.records[0].id, "new");
    }

    #[test]
    fn sources_changed_during_parsing_are_not_cached() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history.jsonl");
        fs::write(&path, b"first").unwrap();
        let store = Mutex::new(Store::at(&dir.path().join("cache.db")).unwrap());
        let first_fingerprint = local::fingerprint(&path).unwrap();
        let parsed = read_cached(&path, &store, || {
            fs::write(&path, b"another write during parsing").unwrap();
            Ok(complete(vec![record(
                &path,
                "partial",
                Provider::Claude,
                10,
            )]))
        })
        .unwrap();
        assert_eq!(parsed.records.len(), 1);
        let store = store.lock().unwrap();
        for (size, modified) in [first_fingerprint, local::fingerprint(&path).unwrap()] {
            assert!(store.cached(&path, size, modified).unwrap().is_none());
        }
    }

    #[test]
    fn incomplete_parses_are_retried_even_when_the_file_is_unchanged() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history.jsonl");
        fs::write(&path, b"incomplete").unwrap();
        let store = Mutex::new(Store::at(&dir.path().join("cache.db")).unwrap());
        let parsed = read_cached(&path, &store, || {
            Ok(history::Parsed {
                records: vec![record(&path, "partial", Provider::Claude, 10)],
                skipped: 1,
            })
        })
        .unwrap();
        assert_eq!(parsed.skipped, 1);
        let retry = read_cached(&path, &store, || {
            Ok(complete(vec![record(
                &path,
                "retried",
                Provider::Claude,
                20,
            )]))
        })
        .unwrap();
        assert_eq!(retry.records[0].id, "retried");
    }

    #[test]
    fn source_removed_after_parsing_still_returns_its_completed_usage() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("history.jsonl");
        fs::write(&path, b"first").unwrap();
        let store = Mutex::new(Store::at(&dir.path().join("cache.db")).unwrap());
        let (size, modified) = local::fingerprint(&path).unwrap();
        let parsed = read_cached(&path, &store, || {
            fs::remove_file(&path).unwrap();
            Ok(complete(vec![record(
                &path,
                "completed",
                Provider::Claude,
                10,
            )]))
        })
        .unwrap();
        assert_eq!(parsed.records[0].id, "completed");
        assert!(
            store
                .lock()
                .unwrap()
                .cached(&path, size, modified)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn incomplete_jsonl_keeps_prior_usage_from_that_source_and_prefers_fresh_records() {
        let dir = TempDir::new().unwrap();
        let projects = dir.path().join("projects");
        fs::create_dir(&projects).unwrap();
        let path = projects.join("session.jsonl");
        let line = serde_json::json!({
            "type": "assistant", "sessionId": "session", "requestId": "request",
            "timestamp": "2026-10-02T13:00:00Z",
            "message": { "id": "message", "model": "fixture-model", "usage": { "input_tokens": 100, "output_tokens": 20 } }
        });
        fs::write(&path, format!("{line}\n{{\"unfinished\":")).unwrap();
        let fresh = history::read_jsonl(&path, Provider::Claude)
            .unwrap()
            .records
            .remove(0);
        let mut old = fresh.clone();
        old.tokens.input = 1;
        let retained = record(&path, "retained", Provider::Claude, 50);
        let removed = record(
            &projects.join("removed.jsonl"),
            "removed",
            Provider::Claude,
            900,
        );
        let result = scan(
            dir.path(),
            Provider::Claude,
            &Config::default(),
            &[old, retained, removed],
        );
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.records[&fresh.id].tokens.input, 100);
        assert!(result.records.contains_key("retained"));
        assert_eq!(result.notices.len(), 1);
    }

    #[test]
    fn unreadable_jsonl_retains_only_its_own_previous_usage() {
        let dir = TempDir::new().unwrap();
        let projects = dir.path().join("projects");
        fs::create_dir(&projects).unwrap();
        let path = projects.join("bad.jsonl");
        fs::write(&path, b"invalid json\n").unwrap();
        let previous = [
            record(&path, "retained", Provider::Claude, 100),
            record(
                &projects.join("removed.jsonl"),
                "removed",
                Provider::Claude,
                200,
            ),
        ];
        let result = scan(dir.path(), Provider::Claude, &Config::default(), &previous);
        assert_eq!(result.records.len(), 1);
        assert!(result.records.contains_key("retained"));
        assert_eq!(result.notices.len(), 1);
    }

    #[test]
    fn failing_cursor_imports_retain_usage_without_reviving_removed_imports() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("missing.csv");
        let config = Config {
            cursor_imports: vec![path.clone()],
            ..Config::default()
        };
        let previous = [
            record(&path, "retained", Provider::Cursor, 100),
            record(
                &dir.path().join("removed.csv"),
                "removed",
                Provider::Cursor,
                200,
            ),
        ];
        let result = scan(dir.path(), Provider::Cursor, &config, &previous);
        assert_eq!(result.records.len(), 1);
        assert!(result.records.contains_key("retained"));
        assert_eq!(result.notices.len(), 1);
    }

    #[test]
    fn successful_empty_sources_and_removed_sources_clear_old_usage() {
        let dir = TempDir::new().unwrap();
        let projects = dir.path().join("projects");
        fs::create_dir(&projects).unwrap();
        let path = projects.join("empty.jsonl");
        fs::write(&path, b"").unwrap();
        let previous = [
            record(&path, "empty", Provider::Claude, 100),
            record(
                &projects.join("removed.jsonl"),
                "removed",
                Provider::Claude,
                200,
            ),
        ];
        let result = scan(dir.path(), Provider::Claude, &Config::default(), &previous);
        assert!(result.records.is_empty());
        assert!(result.notices.is_empty());
    }

    #[test]
    fn database_failures_use_the_same_source_scoped_recovery_policy() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("opencode.db");
        fs::write(&path, b"not a database").unwrap();
        let previous = [
            record(&path, "retained", Provider::OpenCode, 100),
            record(
                &dir.path().join("removed.db"),
                "removed",
                Provider::OpenCode,
                200,
            ),
        ];
        let result = scan(
            dir.path(),
            Provider::OpenCode,
            &Config::default(),
            &previous,
        );
        assert_eq!(result.records.len(), 1);
        assert!(result.records.contains_key("retained"));
        assert_eq!(result.notices.len(), 1);
    }

    #[test]
    fn discovery_failure_preserves_unknown_sources_but_never_overrides_successful_reads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.jsonl");
        let previous = [
            record(&path, "cleared", Provider::Claude, 100),
            record(
                &dir.path().join("unknown.jsonl"),
                "retained",
                Provider::Claude,
                200,
            ),
        ];
        for failure_first in [true, false] {
            let mut result = scan(dir.path(), Provider::Claude, &Config::default(), &[]);
            if failure_first {
                result.discovery_failed(&previous);
            }
            result.accept(&path, Ok(complete(vec![])), &previous);
            if !failure_first {
                result.discovery_failed(&previous);
            }
            assert_eq!(result.records.len(), 1);
            assert!(result.records.contains_key("retained"));
        }
    }
}
