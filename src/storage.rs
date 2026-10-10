use anyhow::{Result, ensure};
use std::{fs, io::Write, path::Path};

pub(crate) fn private_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    ensure!(
        !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "Application data directory must not be a symlink"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("A data directory is required"))?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)?;
    pending.write_all(data)?;
    pending.as_file().sync_all()?;
    // Replace the directory entry, never follow an existing file symlink.
    pending.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomically_replaces_existing_data_without_leftover_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        atomic_write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_targets_are_not_overwritten_and_storage_is_private() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        private_directory(&data).unwrap();
        assert_eq!(
            fs::metadata(&data).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let external = dir.path().join("external");
        fs::write(&external, b"keep").unwrap();
        let link = data.join("settings.json");
        symlink(&external, &link).unwrap();
        atomic_write(&link, b"settings").unwrap();
        assert_eq!(fs::read(&external).unwrap(), b"keep");
        assert_eq!(
            fs::metadata(&link).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let alias = dir.path().join("alias");
        symlink(&data, &alias).unwrap();
        assert!(private_directory(&alias).is_err());
    }
}
