# Open Usage

A native Rust + GPUI dashboard for Codex / ChatGPT, Claude Code, and Cursor. See token history, estimated API-equivalent cost, cache savings, model breakdowns, and subscription allowances in one workspace. Windows and macOS share the same application code.

## Run

Install Rust **1.97.1** (or use `mise install`), then:

```sh
cargo run --bin open-usage
cargo run --bin open-usage -- --demo
```

Windows requires Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK. macOS requires Xcode command-line tools and the macOS SDK. GPUI uses native GPU rendering; there is no webview or JavaScript runtime.

With mise: `mise run dev`, `mise run demo`, `mise run check`, `mise run test`, `mise run lint`, and `mise run package`.

## Connect your usage

The app starts with real local history. Example data is explicitly marked and enabled only with `--demo` or **Connections → Explore example data**.

| Provider | Token history | Account allowances |
| --- | --- | --- |
| Codex / ChatGPT | `CODEX_HOME` or `~/.codex`, including sessions and archived sessions | Existing Codex ChatGPT login; session and weekly/monthly allowances |
| Claude | `CLAUDE_CONFIG_DIR` or `~/.claude/projects`, including subagent history | Claude Code OAuth credentials or the macOS Keychain |
| Cursor | Account dashboard API (90 days), or a usage CSV export | Existing Cursor CLI/desktop login; total, Auto, and API allowance |

Enable **Connections → Account checks** to fetch live quotas and Cursor account history. Account checks are off by default. On macOS, reading an existing Keychain login can trigger the operating system's permission prompt. The app reads credentials only for requests to the corresponding provider's fixed HTTPS endpoints; it does not store, export, or log them. Expired credentials require signing in again with the provider's app or CLI.

**Add data folder** accepts a provider home (for example a custom `.codex` folder), not an individual project. Additional homes contribute token history; live limits use the first discoverable account. Cursor CSV imports replace the previous CSV and take precedence over API history to avoid counting both sources. The imported file is referenced in place, so keep it accessible.

ChatGPT support currently means **Codex usage associated with a ChatGPT subscription**, not ChatGPT website conversation history. Claude support means Claude Code. Cursor account history spans machines; Codex and Claude token history reflects the files on this device.

## Read the numbers

- **Estimated API-equivalent cost** prices local token usage at standard USD model rates. It is not your subscription bill, credits consumed, or an invoice.
- **Reported spend** is a separate value from Cursor's export/API. It is never added to the API-equivalent estimate.
- **Tokens** include uncached input, output, cache reads, and cache writes. Codex's cached input is subtracted from input before aggregation; reasoning is already included in output.
- Repeated Codex cumulative counters and Claude streaming message blocks are reconciled. Incomplete log rows are retried at the next refresh.
- Unknown model IDs remain **Unpriced**, with their tokens retained and partial cost totals clearly marked. **Settings → Model prices** provides a CSV template and imports custom USD-per-million rates for all history.
- The built-in price baseline was checked on **2026-10-02**. It uses standard, short-context rates and 5-minute cache writes. Long-context, 1-hour cache writes, fast-mode premiums, tool charges, and provider-specific discounts can differ. Price overrides can reflect your rates.
- Today starts at local midnight. 7/30/90-day views include today and preceding calendar days. The budget is a guide to estimated cost over the past 30 days.
- Cached quotas are marked. A window that has ended says **Refresh needed**; the app never invents replenished quota.

The sources for baseline prices are [OpenAI pricing](https://developers.openai.com/api/docs/pricing) and [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing). Product inspiration: [OpenUsage](https://www.openusage.ai/) and [t3code's usage documentation](https://github.com/pingdotgg/t3code/blob/54084ae1e6c32809db040e4fa571c80fdf2d8ae4/docs/user/usage.md). This is an independent Rust implementation.

## Everyday controls

Click the tray/menu-bar icon for **Quick glance**. The tray menu opens the dashboard, refreshes usage, or quits. Closing the window minimizes it and keeps background refresh running; use **Quit Open Usage** to exit. The macOS menu-bar title shows today's estimated cost; Windows exposes it in the tray tooltip.

| Shortcut | Action |
| --- | --- |
| Ctrl/Cmd + 1–5 | Overview, Activity, Limits, Connections, Settings |
| Ctrl/Cmd + R | Refresh |
| Ctrl/Cmd + P | Hide amounts, charts, projects, and source paths |
| Ctrl/Cmd + Q | Quit |
| C / T | Cost / token chart |
| L | Limits |
| Tab / Shift+Tab, Enter | Navigate and activate controls |

CSV export includes the selected providers and date range. Export writes the actual data even while display privacy mode is on. No conversations or prompts are exported.

## CLI

```sh
cargo run --no-default-features --bin usage -- --help
cargo run --no-default-features --bin usage -- --refresh --json
cargo run --no-default-features --bin usage -- --online --days 7
```

`--online` enables account checks for that invocation without changing settings. `--demo` never saves example data. The JSON schema (`open-usage.v1`) exposes totals, model breakdowns, provider status, limits, and notices without credential or conversation data. Cached account readings include their check timestamp; consumers should check freshness and reset times.

## Local storage

Settings and the SQLite metadata cache live in `%LOCALAPPDATA%/open-usage` on Windows and `~/Library/Application Support/open-usage` on macOS. `OPEN_USAGE_DATA_DIR` redirects both for testing. The cache stores only usage metadata and source paths. No telemetry, hosted backend, cloud sync, or inference requests are used.

Unchanged history files reuse cached normalized records; changed files are re-read. Manual refresh and automatic refresh (1, 5, or 15 minutes) run off the UI thread. Failed account checks preserve previous readings and mark them cached.

## Build and verify

```sh
cargo fmt --check
cargo check --all-targets
cargo test --no-default-features
cargo clippy --all-targets -- -D warnings
cargo run --bin package
```

Packaging builds the release desktop app and CLI on the current host. Windows produces `dist/Open Usage-windows/`; macOS produces `dist/Open Usage.app` and `dist/usage`. These are local development builds; macOS signing and notarization are not configured. Packaging has no CI or GitHub Actions dependency.

Windows compilation, accounting tests, CLI reads of real local Codex history, and native UI rendering have been verified. macOS source paths and bundling are implemented, but execution and Keychain prompts still need verification on a Mac. Account endpoints are provider-specific and can change; the app presents failures rather than silently fabricating usage.

## Code layout

`model` and `analytics` define normalized usage and filtered totals; `providers/history` reads transcripts and CSV; `providers/live` handles credential discovery and account requests; `store` caches metadata in SQLite; `ui` owns GPUI rendering and native tray controls. The `usage` binary shares the same core, and the Rust `package` binary assembles host-platform artifacts.
