# Open Usage

A native Rust + GPUI app for tracking AI usage, costs, and account limits across ChatGPT (Codex), Claude Code, Cursor, Kiro, Antigravity, and OpenCode.

![Homepage with example data](docs/images/homepage-six-providers-light.jpg)

[Dark theme preview](docs/images/homepage-six-providers-dark.jpg)

## Get started

Install Rust **1.97.1** (or run `mise install`), then launch an optimized build:

```sh
cargo run --release --bin open-usage
```

Windows requires Visual Studio Build Tools with Desktop development with C++ and a Windows SDK. macOS requires Xcode command-line tools.

Sign in through your provider's app or CLI. Open Usage discovers existing logins and local history automatically. Use **Connections** to add a data folder or import a Cursor CSV.

| Provider | Connection |
| --- | --- |
| ChatGPT | Codex login and local session history |
| Claude Code | Claude Code OAuth login and local history |
| Cursor | Cursor desktop/CLI login or dashboard CSV |
| Kiro | Kiro CLI or IDE login; usage is reported in credits |
| Antigravity | IDE/`agy` connection or existing OAuth login; local generation history |
| OpenCode | Local Go/Zen history; a Go key supplies account limits |

Kiro, Antigravity, and OpenCode quota parsing is fixture-tested; signed-in live checks and macOS execution still need validation.

## Using the app

- **Accounts** shows usage totals, current allowances, reset times, and a usage chart. **Limits** shows every reported allowance.
- **Models**, **Activity**, and **Insights** provide breakdowns. Search and filter tables, click column headings to sort, and export Activity to CSV.
- Choose **2 hours, 24 hours, 7 days, 30 days, or 90 days**. The tray's quick view uses Today.
- Adjust automatic refresh, theme, and display privacy in **Settings**. Display privacy hides figures on screen; CSV exports contain actual values.
- Closing the window keeps tracking active when the tray is available. Use **Ctrl/Cmd + Q** to quit and **Ctrl/Cmd + R** to refresh.

## Costs and privacy

Costs use provider-reported amounts when available, otherwise public API-rate estimates from [LiteLLM](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json). Estimates are not subscription bills and exclude subscription fees. Unknown prices stay **-**; Kiro credits stay separate from tokens and dollars. Cached or expired allowances are marked.

History and settings stay on your device. Provider stores are read-only; existing credentials are used for account checks. Prompts, conversations, and usage records are never uploaded. There is no telemetry. Network requests check provider allowances/history and download model prices.

## CLI

```sh
cargo run --release --no-default-features --bin usage -- --online --period 7d --json
```

Use `--offline --refresh` to read local history and cached allowances. Periods: `2h`, `24h`, `7d`, `30d`, `90d`, or `today`. Run with `--help` for options.

Settings and cache live in `%LOCALAPPDATA%/open-usage` on Windows or `~/Library/Application Support/open-usage` on macOS. `OPEN_USAGE_DATA_DIR` overrides that location.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo run --bin package
```

Packaging creates a release build in `dist/` for the current platform. macOS signing and notarization are not configured.

## Credits

Built with [GPUI](https://gpui.rs/) and [gpui-component](https://github.com/longbridge/gpui-component). Provider integrations and design reference [OpenUsage](https://github.com/robinebers/openusage), its [Windows fork](https://github.com/mesomya/openusage-windows), [ai-usagebar](https://github.com/akitaonrails/ai-usagebar), and [t3code](https://github.com/pingdotgg/t3code).

Provider SVGs come from OpenUsage and its Windows fork. Their [MIT notice](docs/licenses/OpenUsage-MIT.txt) and [Windows-fork notice](docs/licenses/OpenUsage-Windows-MIT.txt) are included in packaged builds.
