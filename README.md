# Open Usage

A native Rust + GPUI app for account usage from ChatGPT / Codex, Cursor, and Claude Code. The home page shows connected accounts, subscription allowances, reset times, tokens, and provider-reported usage cost. Model breakdowns have a separate page.

![Open Usage accounts with labeled example data](docs/images/dashboard.jpg)

## Accounting

**There is no model pricing table or token-to-dollar estimate.** Costs come only from the account provider's usage history or export. A missing cost is shown as **— / Not reported**, never $0. Reported zero remains $0.

- Cursor dashboard history/CSV supplies reported usage cost. Claude models used through Cursor belong to the **Cursor account**. They do not imply a Claude subscription.
- Codex and Claude local histories supply token usage. Their subscription endpoints supply allowances and reset times; they do not supply a per-event dollar charge. No subscription fee is inferred from a plan name.
- Provider-reported usage cost can include usage covered by a plan. It is not a subscription invoice, and does not include the subscription fee.
- The Models page keeps the account attached to each model. The same model used through two accounts remains two rows.
- Codex cumulative counters and Claude streaming blocks are reconciled; cached input is counted once. Incomplete rows are retried. No missing tokens are estimated from conversation text.
- Today starts at local midnight. 7/30/90-day views include today and the preceding calendar days.
- Cached and expired allowances are marked. Expired windows require refresh; quota replenishment is never invented.

The Accounts and tray views include accounts with detected history or an active connection. Providers with no account data are shown only in Connections.

## Run

Install Rust **1.97.1** or run `mise install`:

```sh
cargo run --bin open-usage
cargo run --bin open-usage -- --demo
```

Windows requires Visual Studio Build Tools with Desktop development with C++ and a Windows SDK. macOS requires Xcode command-line tools and the macOS SDK. GPUI renders native GPU UI. Longbridge's `gpui-component` 0.5 supplies native controls compatible with GPUI 0.2.2; the application contains no webview or JavaScript runtime.

Mise tasks: `dev`, `demo`, `check`, `test`, `lint`, `build`, `package`.

## Connections

Enable **Connections → Account sync** to read usage from existing signed-in accounts. Account sync is opt-in; the app always reads available local history. No API key is required for subscription logins.

| Account | History | Allowances |
| --- | --- | --- |
| ChatGPT / Codex | `CODEX_HOME` or `~/.codex`, including archived sessions | Existing Codex ChatGPT login |
| Claude Code | `CLAUDE_CONFIG_DIR` or `~/.claude/projects`, including subagents | Claude Code OAuth credentials or macOS Keychain |
| Cursor | Cursor dashboard API, up to 90 days, or a CSV export | Existing Cursor CLI/desktop login |

**Add folder** accepts a provider home such as `.codex`. Additional folders contribute local history; live allowances use the first discoverable login for that provider. Cursor CSV replaces API history to avoid duplication and references the selected file in place.

ChatGPT support means Codex usage associated with the ChatGPT account. Claude support means Claude Code. Cursor account history spans machines; Codex and Claude token histories reflect this device.

Credentials are read only for fixed provider HTTPS endpoints and are never cached, exported, or logged. macOS Keychain may display its own permission prompt. Expired logins require signing in again using the provider's app or CLI.

Example mode is enabled explicitly by `--demo` or **Settings → Example workspace** and never writes example usage to the cache.

## Tray and menu bar

The Windows system tray and macOS menu bar provide:

- Left-click: account quick view.
- Menu: open accounts, quick view, refresh in the background, quit.
- Tooltip: today's tokens and remaining account allowances.
- macOS menu-bar title: remaining account allowances.

Closing the window keeps background tracking active when a tray icon is available. If initialization fails, the app reports the failure in Settings and closing the window quits. **Settings → System tray / Menu bar** reports the actual initialization status.

| Shortcut | Action |
| --- | --- |
| Ctrl/Cmd + 1–6 | Accounts, Activity, Models, Limits, Connections, Settings |
| Ctrl/Cmd + R | Refresh |
| Ctrl/Cmd + P | Display privacy mode |
| Ctrl/Cmd + Q | Quit |
| C / T | Reported-cost / token chart |
| L | Limits |
| Tab / Shift+Tab, Enter | Navigate and activate controls |

CSV export respects the selected account and date range. It includes actual metadata while display privacy mode is enabled, and never includes prompts or credentials.

## CLI and storage

```sh
cargo run --no-default-features --bin usage -- --refresh --json
cargo run --no-default-features --bin usage -- --online --days 7
```

`--online` checks accounts for that invocation without changing preferences. JSON schema **open-usage.v2** includes account summaries, allowances, tokens, models, and `reported_usage_cost_usd`. A missing reported cost is `null`. The old estimate fields have been removed.

Settings and the SQLite metadata cache live in `%LOCALAPPDATA%/open-usage` on Windows and `~/Library/Application Support/open-usage` on macOS. `OPEN_USAGE_DATA_DIR` redirects both for testing. Stored metadata includes source paths, tokens, timestamps and reported usage cost; credentials and conversations are excluded. There is no telemetry or hosted backend.

History scans and account checks run off the UI thread. Automatic refresh supports 1, 5, or 15 minutes. Unchanged files reuse normalized cached records; changed files are re-read. Failed checks preserve prior data and mark account readings cached. Existing settings remain compatible; obsolete pricing overrides are ignored.

## Build and verify

```sh
cargo fmt --check
cargo check --all-targets
cargo test --no-default-features
cargo clippy --all-targets -- -D warnings
cargo run --bin package
```

Packaging builds on the current host: `dist/Open Usage-windows/` on Windows, or `dist/Open Usage.app` and `dist/usage` on macOS. macOS signing/notarization are not configured. There are no GitHub Actions.

Windows compilation, the account dashboard, live Codex/Cursor reads, tray initialization, close-to-tray and quick-view sizing have been verified. Claude history and allowance schemas are fixture-tested. macOS execution, Keychain access and a live Claude account still need validation.

## References

[OpenUsage](https://www.openusage.ai/), [t3code's usage documentation](https://github.com/pingdotgg/t3code/blob/54084ae1e6c32809db040e4fa571c80fdf2d8ae4/docs/user/usage.md), and [Longbridge GPUI components](https://github.com/longbridge/gpui-component) inform the design and provider schemas. This is an independent Rust implementation.
