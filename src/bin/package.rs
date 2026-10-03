//! Host-platform packaging, deliberately run locally rather than in CI.
use anyhow::{Context, Result, bail};
use std::{fs, path::PathBuf, process::Command};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !std::env::args().any(|arg| arg == "--skip-build") {
        let status = Command::new("cargo")
            .args([
                "build",
                "--release",
                "--bin",
                "open-usage",
                "--bin",
                "usage",
            ])
            .current_dir(&root)
            .status()
            .context("Cargo could not be started")?;
        if !status.success() {
            bail!("Release build failed");
        }
    }
    let output = root.join("dist");
    fs::create_dir_all(&output)?;
    match std::env::consts::OS {
        "windows" => {
            let portable = output.join("Open Usage-windows");
            fs::create_dir_all(&portable)?;
            fs::copy(
                root.join("docs/licenses/OpenUsage-MIT.txt"),
                portable.join("OpenUsage-MIT.txt"),
            )?;
            fs::copy(
                root.join("docs/licenses/OpenUsage-Windows-MIT.txt"),
                portable.join("OpenUsage-Windows-MIT.txt"),
            )?;
            fs::copy(
                root.join("target/release/open-usage.exe"),
                portable.join("Open Usage.exe"),
            )?;
            fs::copy(
                root.join("target/release/usage.exe"),
                portable.join("usage.exe"),
            )?;
            println!("Portable app: {}", portable.display());
        }
        "macos" => {
            let bundle = output.join("Open Usage.app");
            let contents = bundle.join("Contents");
            let binaries = contents.join("MacOS");
            let resources = contents.join("Resources");
            fs::create_dir_all(&binaries)?;
            fs::create_dir_all(&resources)?;
            fs::copy(
                root.join("docs/licenses/OpenUsage-MIT.txt"),
                resources.join("OpenUsage-MIT.txt"),
            )?;
            fs::copy(
                root.join("docs/licenses/OpenUsage-Windows-MIT.txt"),
                resources.join("OpenUsage-Windows-MIT.txt"),
            )?;
            fs::copy(
                root.join("target/release/open-usage"),
                binaries.join("open-usage"),
            )?;
            fs::copy(
                root.join("assets/AppIcon.icns"),
                resources.join("AppIcon.icns"),
            )?;
            fs::write(
                contents.join("Info.plist"),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>CFBundleName</key><string>Open Usage</string>\n<key>CFBundleDisplayName</key><string>Open Usage</string>\n<key>CFBundleIdentifier</key><string>dev.chirag.open-usage</string>\n<key>CFBundleExecutable</key><string>open-usage</string>\n<key>CFBundleIconFile</key><string>AppIcon</string>\n<key>CFBundlePackageType</key><string>APPL</string>\n<key>CFBundleShortVersionString</key><string>{}</string>\n<key>CFBundleVersion</key><string>1</string>\n<key>LSMinimumSystemVersion</key><string>13.0</string>\n<key>NSHighResolutionCapable</key><true/>\n</dict></plist>\n",
                    env!("CARGO_PKG_VERSION")
                ),
            )?;
            fs::copy(root.join("target/release/usage"), output.join("usage"))?;
            println!(
                "App bundle: {}\nThis local development bundle is unsigned.",
                bundle.display()
            );
        }
        platform => bail!("Packaging is supported on Windows and macOS, not {platform}"),
    }
    Ok(())
}
