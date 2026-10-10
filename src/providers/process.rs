use serde::Deserialize;
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

// Capture bounded output without logging command lines, which carry a CSRF token.
fn output(program: &str, args: &[&str]) -> Option<String> {
    // Never resolve process-inspection utilities from an untrusted working directory or PATH.
    #[cfg(windows)]
    let mut command = {
        use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
        if program != "powershell.exe" {
            return None;
        }
        let mut directory = [0_u16; 260];
        // SAFETY: the buffer is writable for the size passed to the Windows API.
        let length =
            unsafe { GetSystemDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
        if length == 0 || length >= directory.len() {
            return None;
        }
        let system = std::path::PathBuf::from(String::from_utf16(&directory[..length]).ok()?);
        Command::new(system.join("WindowsPowerShell/v1.0/powershell.exe"))
    };
    #[cfg(not(windows))]
    let mut command = Command::new(match program {
        "ps" => "/bin/ps",
        "lsof" if cfg!(target_os = "macos") => "/usr/sbin/lsof",
        "lsof" => "/usr/bin/lsof",
        "ss" => "/usr/bin/ss",
        _ => return None,
    });
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(524_289).read_to_end(&mut bytes).ok()?;
        (bytes.len() <= 524_288).then_some(bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return String::from_utf8(reader.join().ok()??).ok();
            }
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}
pub struct Server {
    pub csrf: String,
    pub ports: Vec<u16>,
}
#[derive(Deserialize)]
struct Process {
    #[serde(rename = "CommandLine")]
    command: String,
    #[serde(rename = "ProcessId")]
    pid: u32,
}

pub fn flag(command: &str, name: &str) -> Option<String> {
    // Match complete flags, supporting both --flag value and --flag=value.
    let mut words = command.split_whitespace();
    while let Some(word) = words.next() {
        if word == name {
            return words
                .next()
                .map(|value| value.trim_matches(['\'', '"']).to_owned());
        }
        if let Some(value) = word.strip_prefix(&format!("{name}=")) {
            return Some(value.trim_matches(['\'', '"']).to_owned());
        }
    }
    None
}

pub fn servers() -> Vec<Server> {
    let mut processes: Vec<Process> = vec![];
    if cfg!(windows) {
        // Piping a single process to ConvertTo-Json produces an object, not an array.
        let script = "ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_Process -Filter \"Name LIKE 'language_server%' OR Name='agy.exe'\" | Where-Object { $_.CommandLine } | Select-Object ProcessId,CommandLine)";
        if let Some(raw) = output(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", script],
        ) {
            processes = serde_json::from_str(&raw).unwrap_or_default();
        }
    } else if let Some(raw) = output("ps", &["-ax", "-o", "pid=,command="]) {
        for line in raw.lines() {
            let line = line.trim();
            if let Some((pid, command)) = line.split_once(char::is_whitespace)
                && let Ok(pid) = pid.parse()
            {
                processes.push(Process {
                    pid,
                    command: command.trim().into(),
                });
            }
        }
    }
    let mut servers = vec![];
    for process in processes {
        let lower = process.command.to_lowercase();
        let executable = executable(&lower);
        let agy = executable
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| matches!(name, "agy" | "agy.exe"));
        if !agy && !(lower.contains("language_server") && lower.contains("antigravity")) {
            continue;
        }
        let csrf = flag(&process.command, "--csrf_token").unwrap_or_default();
        if !agy && csrf.is_empty() {
            continue;
        }
        let mut ports = vec![];
        if cfg!(windows) {
            let script = format!(
                "Get-NetTCPConnection -State Listen -OwningProcess {} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty LocalPort",
                process.pid
            );
            if let Some(raw) = output(
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", &script],
            ) {
                ports.extend(
                    raw.lines()
                        .filter_map(|line| line.trim().parse::<u16>().ok()),
                );
            }
        } else {
            let pid = process.pid.to_string();
            if let Some(raw) = output("lsof", &["-a", "-p", &pid, "-iTCP", "-sTCP:LISTEN", "-Fn"]) {
                ports.extend(
                    raw.lines()
                        .filter(|line| line.starts_with('n'))
                        .filter_map(|line| line.rsplit(':').next()?.parse::<u16>().ok()),
                );
            } else if let Some(raw) = output("ss", &["-ltnp"]) {
                for line in raw
                    .lines()
                    .filter(|line| line.contains(&format!("pid={pid},")))
                {
                    if let Some(port) = line
                        .split_whitespace()
                        .nth(3)
                        .and_then(|address| address.rsplit(':').next())
                        .and_then(|port| port.parse::<u16>().ok())
                    {
                        ports.push(port);
                    }
                }
            }
        }
        if let Some(port) =
            flag(&process.command, "--extension_server_port").and_then(|port| port.parse().ok())
        {
            ports.push(port);
        }
        ports.retain(|port| *port != 0);
        ports.sort();
        ports.dedup();
        ports.truncate(8);
        if !ports.is_empty() {
            servers.push(Server { csrf, ports });
        }
        if servers.len() >= 3 {
            break;
        }
    }
    servers
}
fn executable(command: &str) -> &str {
    let command = command.trim();
    if let Some(quoted) = command.strip_prefix('"') {
        quoted.split('"').next().unwrap_or("")
    } else {
        command.split_whitespace().next().unwrap_or("")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_paths_preserve_quoted_spaces() {
        assert_eq!(
            executable(r#""C:\Users\Some User\agy.exe" --extension_server_port 12345"#),
            r"C:\Users\Some User\agy.exe"
        );
        assert_eq!(executable("/usr/bin/agy --flag value"), "/usr/bin/agy");
    }
    #[cfg(windows)]
    #[test]
    fn windows_process_discovery_preserves_zero_one_and_multiple_results() {
        for count in 0..=2 {
            let script = format!(
                "ConvertTo-Json -Compress -InputObject @(for ($i=0; $i -lt {count}; $i++) {{ [pscustomobject]@{{ ProcessId=($i+1); CommandLine='fixture' }} }})"
            );
            let raw = output(
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", &script],
            )
            .unwrap();
            let processes: Vec<Process> = serde_json::from_str(&raw).unwrap();
            assert_eq!(processes.len(), count);
        }
    }
    #[test]
    fn flags_match_complete_names_and_support_equals() {
        assert_eq!(
            flag(
                "language_server --csrf_token='fixture-token' --extension_server_port 12345",
                "--csrf_token"
            )
            .as_deref(),
            Some("fixture-token")
        );
        assert_eq!(
            flag(
                "language_server --extension_server_port=12345",
                "--extension_server_port"
            )
            .as_deref(),
            Some("12345")
        );
        assert!(
            flag(
                "--not_csrf_token value --csrf_token_other value",
                "--csrf_token"
            )
            .is_none()
        );
    }
}
