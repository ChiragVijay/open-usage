use crate::model::Provider;
use std::{collections::BTreeSet, path::PathBuf};

/// Discovery uses filesystem evidence only; it never launches an agent or reads credentials.
pub struct Environment {
    pub home: PathBuf,
    pub path: Vec<PathBuf>,
    pub local_data: PathBuf,
    pub program_dirs: Vec<PathBuf>,
    pub applications: Vec<PathBuf>,
}
impl Environment {
    pub fn current() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            path: std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
            local_data: dirs::data_local_dir().unwrap_or_default(),
            program_dirs: ["ProgramFiles", "ProgramFiles(x86)"]
                .into_iter()
                .filter_map(std::env::var_os)
                .map(PathBuf::from)
                .collect(),
            applications: vec![PathBuf::from("/Applications"), home.join("Applications")],
            home,
        }
    }
}

pub fn detect(provider: Provider, homes: &[PathBuf], env: &Environment) -> Vec<String> {
    let mut evidence = BTreeSet::new();
    let markers: &[&str] = match provider {
        Provider::Codex => &["auth.json", "config.toml", "sessions", "archived_sessions"],
        Provider::Claude => &[".credentials.json", "settings.json", "projects"],
        Provider::Cursor => &[
            "auth.json",
            "cli-config.json",
            "User/globalStorage/state.vscdb",
            "User/settings.json",
        ],
        Provider::Kiro => &[
            "data.sqlite3",
            "kiro-auth-token.json",
            "settings/cli.json",
            "User/globalStorage/state.vscdb",
        ],
        Provider::Antigravity => &[
            "conversations",
            "antigravity-oauth-token",
            "User/globalStorage/state.vscdb",
        ],
        Provider::OpenCode => &[
            "auth.json",
            "opencode.db",
            "opencode-next.db",
            "storage/message",
        ],
    };
    for home in homes {
        if markers.iter().any(|marker| home.join(marker).exists())
            || (provider == Provider::OpenCode
                && super::local::files(home, "opencode", ".db")
                    .is_ok_and(|files| !files.is_empty()))
        {
            evidence.insert(home.clone());
        }
    }
    let commands: &[&str] = match provider {
        Provider::Codex => &["codex"],
        Provider::Claude => &["claude"],
        Provider::Cursor => &["cursor", "cursor-agent", "agent"],
        Provider::Kiro => &["kiro", "kiro-cli"],
        Provider::Antigravity => &["antigravity", "agy"],
        Provider::OpenCode => &["opencode"],
    };
    let search = env.path.iter().cloned().chain([
        env.home.join(".local/bin"),
        env.home.join(".cargo/bin"),
        env.home.join(".claude/local"),
        env.home.join(".cursor/bin"),
    ]);
    for directory in search {
        for command in commands {
            for extension in if cfg!(windows) {
                &["exe", "cmd", "ps1", "bat", ""][..]
            } else {
                &[""][..]
            } {
                let candidate = directory.join(if extension.is_empty() {
                    (*command).to_owned()
                } else {
                    format!("{command}.{extension}")
                });
                // Generic `agent` is only a Cursor signal when its path identifies Cursor.
                if candidate.is_file()
                    && (*command != "agent"
                        || candidate
                            .canonicalize()
                            .unwrap_or_else(|_| candidate.clone())
                            .to_string_lossy()
                            .to_lowercase()
                            .contains("cursor"))
                {
                    evidence.insert(candidate);
                }
            }
        }
    }
    if matches!(
        provider,
        Provider::Cursor | Provider::Kiro | Provider::Antigravity | Provider::OpenCode
    ) {
        for root in env
            .program_dirs
            .iter()
            .cloned()
            .chain([env.local_data.join("Programs")])
        {
            let candidate = root.join(format!("{0}/{0}.exe", provider.name()));
            if candidate.is_file() {
                evidence.insert(candidate);
            }
        }
    }
    if provider == Provider::Codex {
        // Store installations may have no CLI alias or history until first launch.
        for root in &env.program_dirs {
            if let Ok(packages) = std::fs::read_dir(root.join("WindowsApps")) {
                for package in packages.flatten() {
                    if package
                        .file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .starts_with("openai.codex_")
                        && package.path().is_dir()
                    {
                        evidence.insert(package.path());
                    }
                }
            }
        }
    }
    let bundle = match provider {
        Provider::Codex => "Codex.app",
        Provider::Claude => "", // Claude Desktop is distinct from Claude Code.
        Provider::Cursor => "Cursor.app",
        Provider::Kiro => "Kiro.app",
        Provider::Antigravity => "Antigravity.app",
        Provider::OpenCode => "OpenCode.app",
    };
    if !bundle.is_empty() {
        for root in &env.applications {
            let candidate = root.join(bundle);
            if candidate.is_dir() {
                evidence.insert(candidate);
            }
        }
    }
    evidence
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}
