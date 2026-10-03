# Homepage layout proposal

Complete light and dark examples for Open Usage's Accounts homepage, generated with the built-in imagegen tool. All numbers, plans, connection states, and reset times are illustrative example data.

- [Light example](homepage-light-v1.png)
- [Dark example](homepage-dark-v1.png)

## What users should understand immediately

The selected period's total tokens, usage cost, and sessions form a compact summary. Three account panels then put each provider's current remaining allowance and reset times together. Low allowance has both a text label and a color cue. A single chart below the accounts shows the usage pattern over the selected period.

The period selector affects usage totals and the chart. Allowances reflect the current provider windows, labelled separately as current allowances. The homepage includes only accounts actually discovered or connected; the three panels here illustrate a fully populated state.

## Cost clarity

The example includes $37.40 of reported usage and $46.80 of API-equivalent estimates, for $84.20 total. Each account states whether its cost is reported or estimated. These amounts exclude subscription fees. Unknown prices and missing allowances should retain explicit missing-data states when implemented.

## Layout and theme

Keep the existing sidebar and native titlebar. Use quiet borders, consistent spacing, aligned account panels, tabular numbers, and readable secondary text. Retain the same information hierarchy in both themes. On narrower desktop windows, reflow the panels to preserve text and control readability.

Generation images are design references. Production progress bars and chart geometry should be calculated from the actual data; warning badges should not shorten progress-bar tracks. Stale readings must retain their cached status instead of displaying Live.

## Generation prompts

- [Complete light prompt](homepage-light-prompt.txt)
- [Complete dark prompt](homepage-dark-prompt.txt)

The light design was grounded in the repository's existing dashboard screenshot. The dark image was generated from the light example to preserve its layout. Both images were visually checked for complete account sections, cost labels, the chart, and footer. The original images are design references; the implementation and captures below reflect the finished native UI.

## Implemented layout

The homepage supports six accounts: ChatGPT, Claude Code, Cursor, Kiro, Antigravity, and OpenCode. Desktop windows show them together with the summary and chart. Wide windows use cards with additional allowances when they fit; narrower or shorter windows use compact rows with two primary meters and reset times. There are no account or allowance arrows. Limits keeps every reported window available. When accounts exceed the viewport budget, only the account region scrolls. Kiro shows credit balances and leaves unreported tokens and dollars empty.

Models and Activity have model/account search (plus projects in Activity), account selection, cost-source selection, Clear, and ascending/descending column sorting. Missing cost remains last in either direction. Activity sorts and exports the full filtered dataset before pagination. Model sessions count distinct session identifiers per account and model.

Actual native captures use isolated, illustrative data:

- [Desktop homepage](../images/homepage-responsive-light.png)
- [Compact homepage](../images/homepage-responsive-small.png)
- [Dark homepage](../images/homepage-responsive-dark.png)
- [Models table](../images/models-sort-filter.png)
- [Six-provider homepage](../images/homepage-six-providers-light.jpg)
- [Six-provider dark homepage](../images/homepage-six-providers-dark.jpg)

References: [OpenUsage dashboard design](https://github.com/robinebers/openusage/blob/main/docs/dashboard.md) for provider meters and reset times, and [t3code usage](https://github.com/pingdotgg/t3code/blob/main/docs/user/usage.md) for model, token and cost filtering. Allowances remain separate by account.
