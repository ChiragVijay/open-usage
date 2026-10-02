use crate::{
    config::data_dir,
    model::{Snapshot, UsageRecord},
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::{fs, path::Path};

pub struct Store(Connection);
impl Store {
    pub fn open() -> Result<Self> {
        fs::create_dir_all(data_dir())?;
        Self::at(&data_dir().join("usage.db"))
    }
    pub fn at(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
            CREATE TABLE IF NOT EXISTS snapshot (id INTEGER PRIMARY KEY CHECK (id=1), json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, size INTEGER NOT NULL, modified INTEGER NOT NULL, records TEXT NOT NULL);")?;
        Ok(Self(connection))
    }
    pub fn load(&self) -> Result<Option<Snapshot>> {
        let value: Option<String> = self
            .0
            .query_row("SELECT json FROM snapshot WHERE id=1", [], |row| row.get(0))
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
    }
    pub fn save(&self, snapshot: &Snapshot) -> Result<()> {
        self.0.execute(
            "INSERT INTO snapshot VALUES(1, ?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
            [serde_json::to_string(snapshot)?],
        )?;
        Ok(())
    }
    pub fn cached(
        &self,
        path: &Path,
        size: u64,
        modified: u64,
    ) -> Result<Option<Vec<UsageRecord>>> {
        let value: Option<String> = self
            .0
            .query_row(
                "SELECT records FROM files WHERE path=?1 AND size=?2 AND modified=?3",
                params![path.to_string_lossy(), size as i64, modified as i64],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
    }
    pub fn cache(
        &self,
        path: &Path,
        size: u64,
        modified: u64,
        records: &[UsageRecord],
    ) -> Result<()> {
        self.0.execute("INSERT INTO files VALUES(?1,?2,?3,?4) ON CONFLICT(path) DO UPDATE SET size=excluded.size,modified=excluded.modified,records=excluded.records", params![path.to_string_lossy(), size as i64, modified as i64, serde_json::to_string(records)?])?;
        Ok(())
    }
}
