//! Read-only helpers for agent-owned databases. Never create a missing source.
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub fn database(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(2))?;
    Ok(connection)
}
/// Include WAL changes: the main DB's modification time alone misses live writes.
pub fn fingerprint(path: &Path) -> Result<(u64, u64)> {
    use sha2::{Digest, Sha256};
    use std::time::UNIX_EPOCH;
    let mut digest = Sha256::new();
    let mut size = 0u64;
    for source in [
        path.to_path_buf(),
        std::path::PathBuf::from(format!("{}-wal", path.to_string_lossy())),
    ] {
        match std::fs::metadata(&source) {
            Ok(metadata) => {
                size = size.saturating_add(metadata.len());
                digest.update(metadata.len().to_le_bytes());
                digest.update(
                    metadata
                        .modified()?
                        .duration_since(UNIX_EPOCH)?
                        .as_nanos()
                        .to_le_bytes(),
                );
            }
            Err(error) if source != path && error.kind() == std::io::ErrorKind::NotFound => {
                digest.update([0])
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok((
        size,
        u64::from_le_bytes(digest.finalize()[..8].try_into().unwrap()),
    ))
}
pub fn table(connection: &Connection, name: &str) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )?)
}
pub fn files(root: &Path, prefix: &str, suffix: &str) -> Result<Vec<std::path::PathBuf>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    };
    let mut paths = vec![];
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(prefix) && name.ends_with(suffix) && entry.path().is_file() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_fingerprint_includes_live_wal_changes() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("agent.db");
        std::fs::write(&path, b"same-main-db").unwrap();
        let first = fingerprint(&path).unwrap();
        let wal = dir.path().join("agent.db-wal");
        std::fs::write(&wal, b"new-row").unwrap();
        let second = fingerprint(&path).unwrap();
        assert_ne!(first, second);
        std::fs::write(&wal, b"another-live-row").unwrap();
        assert_ne!(second, fingerprint(&path).unwrap());
    }
}
