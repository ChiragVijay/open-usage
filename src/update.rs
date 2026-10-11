//! Compares this build with the latest stable GitHub release and applies it.
use serde::Deserialize;
#[cfg(any(test, windows, target_os = "macos"))]
use std::path::{Path, PathBuf};
use std::time::Duration;
#[cfg(any(windows, target_os = "macos"))]
use std::{
    fs::{self, File},
    io,
    process::{Command, Stdio},
};

const LATEST: &str = "https://api.github.com/repos/ChiragVijay/open-usage/releases/latest";
const WINDOWS_ASSET: &str = "open-usage.exe";
const MAC_ASSET: &str = "open-usage.dmg";
#[cfg(any(windows, target_os = "macos"))]
const OPENED: &str =
    "The update was downloaded. It could not replace this copy, so the file was opened.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub version: String,
    pub url: String,
}

#[derive(Debug)]
pub enum Check {
    Current,
    Available(Offer),
    /// A newer release exists, without the download for this system.
    MissingDownload {
        version: String,
    },
    Unsupported,
}

pub enum Install {
    /// A helper is waiting for this process to exit, then it replaces and relaunches.
    Restarting,
    Opened(String),
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn platform_asset() -> Option<&'static str> {
    if cfg!(windows) {
        Some(WINDOWS_ASSET)
    } else if cfg!(target_os = "macos") {
        Some(MAC_ASSET)
    } else {
        None
    }
}

/// Ask on a launch when a newer stable version exists. A delay is not a skip:
/// the saved choice is ignored here, so the next launch asks again.
pub fn prompt_on_launch(latest: &str, current: &str, already_asked: bool) -> bool {
    if already_asked {
        return false;
    }
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim().strip_prefix('v').unwrap_or(text.trim());
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() || text.is_empty() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn check() -> Result<Check, String> {
    let Some(asset) = platform_asset() else {
        return Ok(Check::Unsupported);
    };
    let body = fetch_latest()?;
    if body.is_empty() {
        return Ok(Check::Current);
    }
    decide(&body, current_version(), asset)
}

pub fn install(offer: &Offer) -> Result<Install, String> {
    if !trusted_download(&offer.url) {
        return Err("The release download is not from this repository.".into());
    }
    #[cfg(windows)]
    return install_windows(offer);
    #[cfg(target_os = "macos")]
    return install_macos(offer);
    #[cfg(not(any(windows, target_os = "macos")))]
    Err("Updates run on Windows and macOS.".into())
}

pub(crate) fn decide(body: &str, current: &str, asset: &str) -> Result<Check, String> {
    let release: GithubRelease = serde_json::from_str(body)
        .map_err(|_| "GitHub returned an unreadable release.".to_string())?;
    if release.draft || release.prerelease {
        return Ok(Check::Current);
    }
    let Some(latest) = parse_version(&release.tag_name) else {
        return Ok(Check::Current);
    };
    let Some(running) = parse_version(current) else {
        return Err("The app version could not be read.".into());
    };
    if latest <= running {
        return Ok(Check::Current);
    }
    let version = format!("{}.{}.{}", latest.0, latest.1, latest.2);
    let Some(asset) = release.assets.iter().find(|item| item.name == asset) else {
        return Ok(Check::MissingDownload { version });
    };
    if !trusted_download(&asset.browser_download_url) {
        return Err("The release download is not from this repository.".into());
    }
    Ok(Check::Available(Offer {
        version,
        url: asset.browser_download_url.clone(),
    }))
}

fn trusted_download(url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url
            .path()
            .starts_with("/ChiragVijay/open-usage/releases/download/")
}

fn fetch_latest() -> Result<String, String> {
    let response = client()?
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|_| "GitHub could not be reached.".to_string())?;
    if response.status().as_u16() == 404 {
        return Ok(String::new());
    }
    if !response.status().is_success() {
        return Err("GitHub did not return the latest release.".into());
    }
    let bytes = crate::input::read(response, crate::input::RESPONSE_BYTES)
        .map_err(|_| "GitHub returned an unreadable release.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "GitHub returned an unreadable release.".to_string())
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent(concat!("OpenUsage/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "The update client could not start.".into())
}

#[cfg(any(windows, target_os = "macos"))]
fn download(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| "The update folder could not be created.")?;
    }
    let mut response = client()?
        .get(url)
        .send()
        .map_err(|_| "The update could not be downloaded.")?;
    if !response.status().is_success() {
        return Err("The update could not be downloaded.".into());
    }
    let partial = dest.with_extension("partial");
    let mut file = File::create(&partial).map_err(|_| "The update could not be saved.")?;
    io::copy(&mut response, &mut file).map_err(|_| "The update could not be saved.")?;
    file.sync_all().ok();
    drop(file);
    fs::rename(&partial, dest).map_err(|_| "The update could not be saved.")?;
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
fn update_dir() -> Result<PathBuf, String> {
    let path = crate::config::data_dir()
        .map_err(|_| "No folder is available for the update.".to_string())?
        .join("updates");
    fs::create_dir_all(&path).map_err(|_| "The update folder could not be created.")?;
    Ok(path)
}

#[cfg(any(windows, target_os = "macos"))]
fn directory_writable(dir: &Path) -> bool {
    let probe = dir.join(".open-usage-write-probe");
    fs::write(&probe, b"").is_ok() && fs::remove_file(&probe).is_ok()
}

#[cfg(any(test, target_os = "macos"))]
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()? != "Contents" {
        return None;
    }
    let bundle = contents.parent()?;
    if bundle.extension()? != "app" {
        return None;
    }
    Some(bundle.to_path_buf())
}

#[cfg(any(windows, target_os = "macos"))]
fn open_file(path: &Path) -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "Start-Process -LiteralPath $env:OPEN_USAGE_FILE",
            ])
            .env("OPEN_USAGE_FILE", path);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("/usr/bin/open");
        command.arg(path);
        command
    } else {
        return Err("The update was downloaded, but it could not be opened.".into());
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| "The update was downloaded, but it could not be opened.".to_string())
}

#[cfg(any(windows, target_os = "macos"))]
fn opened(path: &Path) -> Result<Install, String> {
    open_file(path)?;
    Ok(Install::Opened(OPENED.into()))
}

#[cfg(windows)]
fn install_windows(offer: &Offer) -> Result<Install, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let exe = std::env::current_exe().map_err(|_| "The running program could not be found.")?;
    let parent = exe
        .parent()
        .filter(|path| directory_writable(path))
        .map(Path::to_path_buf);
    let source = if let Some(parent) = parent {
        parent.join("open-usage.update.exe")
    } else {
        update_dir()?.join(WINDOWS_ASSET)
    };
    download(&offer.url, &source)?;
    if !directory_writable(exe.parent().unwrap_or(Path::new("."))) {
        return opened(&source);
    }
    let script_path = update_dir()?.join("replace.ps1");
    if fs::write(&script_path, WINDOWS_REPLACE).is_err() {
        return opened(&source);
    }
    let spawned = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script_path)
        .arg(std::process::id().to_string())
        .arg(&source)
        .arg(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if spawned.is_err() {
        return opened(&source);
    }
    Ok(Install::Restarting)
}

#[cfg(target_os = "macos")]
fn install_macos(offer: &Offer) -> Result<Install, String> {
    use std::os::unix::process::CommandExt;
    let dir = update_dir()?;
    let dmg = dir.join(MAC_ASSET);
    download(&offer.url, &dmg)?;
    let Some(bundle) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(app_bundle)
        .filter(|bundle| {
            bundle
                .parent()
                .is_some_and(|parent| directory_writable(parent))
        })
    else {
        return opened(&dmg);
    };
    let Some(staged) = stage_app(&dmg, &dir) else {
        return opened(&dmg);
    };
    let script_path = dir.join("replace.sh");
    if fs::write(&script_path, MAC_REPLACE).is_err() {
        return opened(&dmg);
    }
    let spawned = Command::new("/bin/sh")
        .arg(&script_path)
        .arg(std::process::id().to_string())
        .arg(&staged)
        .arg(&bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    if spawned.is_err() {
        return opened(&dmg);
    }
    Ok(Install::Restarting)
}

#[cfg(target_os = "macos")]
fn stage_app(dmg: &Path, dir: &Path) -> Option<PathBuf> {
    let mount = dir.join(format!("mount-{}", std::process::id()));
    let _ = fs::remove_dir_all(&mount);
    fs::create_dir_all(&mount).ok()?;
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    let staged = dir.join("stage").join("Open Usage.app");
    let copied = attached && {
        let _ = fs::remove_dir_all(dir.join("stage"));
        fs::create_dir_all(dir.join("stage")).is_ok()
            && Command::new("/usr/bin/ditto")
                .arg(mount.join("Open Usage.app"))
                .arg(&staged)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
            && staged.join("Contents/MacOS/open-usage").is_file()
    };
    let _ = Command::new("/usr/bin/hdiutil")
        .args(["detach", "-force"])
        .arg(&mount)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = fs::remove_dir_all(&mount);
    if copied { Some(staged) } else { None }
}

#[cfg(windows)]
const WINDOWS_REPLACE: &str = r#"param([string]$ProcessId, [string]$Source, [string]$Destination)
$ErrorActionPreference = "Stop"
Wait-Process -Id ([int]$ProcessId) -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500
try {
  Move-Item -LiteralPath $Source -Destination $Destination -Force
  Start-Process -FilePath $Destination
} catch {
  Start-Process -FilePath $Source
}
Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force -ErrorAction SilentlyContinue
"#;

#[cfg(target_os = "macos")]
const MAC_REPLACE: &str = r#"#!/bin/sh
pid="$1"
staged="$2"
target="$3"
while kill -0 "$pid" 2>/dev/null; do
  sleep 1
done
sleep 1
rm -rf "$target"
if /usr/bin/ditto "$staged" "$target"; then
  /usr/bin/open "$target"
else
  /usr/bin/open "$staged"
fi
rm -f "$0"
"#;

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, prerelease: bool, assets: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","draft":{draft},"prerelease":{prerelease},"assets":[{assets}]}}"#
        )
    }

    fn asset(name: &str) -> String {
        format!(
            r#"{{"name":"{name}","browser_download_url":"https://github.com/ChiragVijay/open-usage/releases/download/v0.3.0/{name}"}}"#
        )
    }

    #[test]
    fn versions_compare_numerically_and_accept_a_v_prefix() {
        assert_eq!(parse_version("v0.2.0"), Some((0, 2, 0)));
        assert_eq!(parse_version("0.2.0"), Some((0, 2, 0)));
        assert!(parse_version("0.10.0") > parse_version("0.9.0"));
        assert_eq!(parse_version("0.2.0-beta"), None);
        assert_eq!(parse_version("v1"), None);
    }

    #[test]
    fn delay_does_not_skip_a_version_on_the_next_launch() {
        assert!(prompt_on_launch("0.3.0", "0.2.0", false));
        assert!(prompt_on_launch("v0.3.0", "0.2.0", false));
        assert!(!prompt_on_launch("0.3.0", "0.2.0", true));
        assert!(!prompt_on_launch("0.2.0", "0.2.0", false));
        assert!(!prompt_on_launch("0.1.9", "0.2.0", false));
        let mut config = crate::config::Config::default();
        config.update_delayed = Some("0.3.0".into());
        let loaded: crate::config::Config =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(loaded.update_delayed.as_deref(), Some("0.3.0"));
        let fresh: crate::config::Config = serde_json::from_str("{}").unwrap();
        assert_eq!(fresh.update_delayed, None);
        assert!(prompt_on_launch(
            loaded.update_delayed.as_deref().unwrap(),
            "0.2.0",
            false
        ));
    }

    #[test]
    fn latest_stable_release_selects_the_platform_asset() {
        let body = release(
            "v0.3.0",
            false,
            false,
            &format!("{},{}", asset("open-usage.exe"), asset("open-usage.dmg")),
        );
        let exe = decide(&body, "0.2.0", WINDOWS_ASSET).unwrap();
        match exe {
            Check::Available(offer) => {
                assert_eq!(offer.version, "0.3.0");
                assert!(offer.url.ends_with("/open-usage.exe"));
            }
            _ => panic!("expected the exe"),
        }
        let dmg = decide(&body, "0.2.0", MAC_ASSET).unwrap();
        assert!(matches!(dmg, Check::Available(offer) if offer.url.ends_with("/open-usage.dmg")));
        assert!(matches!(
            decide(&body, "0.3.0", WINDOWS_ASSET).unwrap(),
            Check::Current
        ));
        assert!(matches!(
            decide(&body, "0.4.0", WINDOWS_ASSET).unwrap(),
            Check::Current
        ));
        assert!(matches!(
            decide(
                &release("v0.9.0", true, false, &asset(WINDOWS_ASSET)),
                "0.2.0",
                WINDOWS_ASSET
            )
            .unwrap(),
            Check::Current
        ));
        assert!(matches!(
            decide(
                &release("v0.9.0", false, true, &asset(WINDOWS_ASSET)),
                "0.2.0",
                WINDOWS_ASSET
            )
            .unwrap(),
            Check::Current
        ));
        match decide(
            &release("v0.9.0", false, false, &asset(MAC_ASSET)),
            "0.2.0",
            WINDOWS_ASSET,
        )
        .unwrap()
        {
            Check::MissingDownload { version } => assert_eq!(version, "0.9.0"),
            other => panic!("expected a missing download, got {other:?}"),
        }
    }

    #[test]
    fn downloads_must_come_from_this_repository() {
        assert!(trusted_download(
            "https://github.com/ChiragVijay/open-usage/releases/download/v0.3.0/open-usage.exe"
        ));
        assert!(!trusted_download(
            "https://example.com/ChiragVijay/open-usage/releases/download/v0.3.0/open-usage.exe"
        ));
        assert!(!trusted_download(
            "https://github.com/other/open-usage/releases/download/v0.3.0/open-usage.exe"
        ));
        assert_eq!(
            LATEST,
            "https://api.github.com/repos/ChiragVijay/open-usage/releases/latest"
        );
    }

    #[test]
    fn an_app_bundle_is_the_folder_around_the_running_executable() {
        let bundle = app_bundle(Path::new(
            "/Applications/Open Usage.app/Contents/MacOS/open-usage",
        ))
        .unwrap();
        assert_eq!(bundle, Path::new("/Applications/Open Usage.app"));
        assert!(app_bundle(Path::new("/tmp/open-usage")).is_none());
    }
}
