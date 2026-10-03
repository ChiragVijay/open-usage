use super::*;
use chrono::Local;
use open_usage::{
    analytics::{Analysis, Totals},
    model::{LimitWindow, Provider, ProviderStatus, UsageRecord},
};

impl Dashboard {
    pub(super) fn sidebar(&self, rail: bool, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .w(px(if rail { 56. } else { 188. }))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(theme.sidebar))
            .border_r_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(layout::HEADER_HEIGHT))
                    .px(px(if rail { 16. } else { 18. }))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .size(px(22.))
                            .rounded(px(4.))
                            .bg(rgb(theme.ink))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                icon("brand", theme)
                                    .size(px(15.))
                                    .text_color(rgb(theme.sidebar)),
                            ),
                    )
                    .when(!rail, |row| {
                        row.child(title("Open Usage", theme).text_size(px(15.)))
                    }),
            )
            .child(
                div()
                    .px(px(if rail { 8. } else { 12. }))
                    .pt(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .children(
                        [Page::Overview, Page::Models, Page::Limits, Page::Activity]
                            .into_iter()
                            .map(|page| self.navigation(page, rail, cx)),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px(px(if rail { 8. } else { 12. }))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(self.navigation(Page::Connections, rail, cx))
                    .child(self.navigation(Page::Settings, rail, cx)),
            )
            .when(!rail, |sidebar| {
                sidebar.child(
                    div()
                        .border_t_1()
                        .border_color(rgb(theme.line))
                        .px(px(18.))
                        .py(px(16.))
                        .child(
                            label(concat!("v", env!("CARGO_PKG_VERSION")), theme)
                                .text_size(px(11.)),
                        ),
                )
            })
    }
    fn navigation(&self, page: Page, rail: bool, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme();
        let active = self.page == page;
        div()
            .id(SharedString::from(format!("nav-{}", page.title())))
            .focusable()
            .tab_index(0)
            .h(px(36.))
            .rounded(px(4.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
            .cursor_pointer()
            .bg(rgb(if active { theme.wash } else { theme.sidebar }))
            .text_color(rgb(if active { theme.accent } else { theme.muted }))
            .font_weight(if active {
                FontWeight::MEDIUM
            } else {
                FontWeight::NORMAL
            })
            .text_size(px(13.))
            .hover(move |style| style.bg(rgb(theme.wash)))
            .focus(move |style| style.border_1().border_color(rgb(theme.accent)))
            .child(
                icon(page.icon(), theme)
                    .size(px(17.))
                    .text_color(rgb(if active { theme.accent } else { theme.muted })),
            )
            .when(!rail, |item| item.child(page.title()))
            .tooltip(move |window, cx| {
                gpui_component::tooltip::Tooltip::new(page.title()).build(window, cx)
            })
            .on_click(cx.listener(move |view, _, window, cx| {
                view.perform(UiAction::Page(page), window, cx)
            }))
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    view.perform(UiAction::Page(page), window, cx);
                    cx.stop_propagation();
                }
            }))
    }
    pub(super) fn header(&self, narrow: bool, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let updated = self
            .data()
            .refreshed_at
            .map(|time| format!("Updated {}", time.with_timezone(&Local).format("%H:%M")))
            .unwrap_or_else(|| "Not synced".into());
        div()
            .h(px(layout::HEADER_HEIGHT))
            .flex_shrink_0()
            .px(px(if narrow { 12. } else { 24. }))
            .bg(rgb(theme.bg))
            .border_b_1()
            .border_color(rgb(theme.line))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(title(self.page.title(), theme).text_size(px(16.)))
            .child(div().flex_1())
            .when(!narrow, |row| {
                row.child(
                    label(
                        if self.refreshing {
                            "Refreshing…".into()
                        } else {
                            updated
                        },
                        theme,
                    )
                    .text_size(px(12.)),
                )
            })
            .child(
                self.button(
                    "header-theme",
                    if narrow {
                        ""
                    } else if self.config.dark {
                        "Dark"
                    } else {
                        "Light"
                    },
                    Some("theme"),
                    false,
                    UiAction::Dark,
                    cx,
                )
                .tooltip("Switch theme"),
            )
            .child(
                self.button(
                    "header-refresh",
                    if narrow { "" } else { "Refresh" },
                    Some("refresh"),
                    false,
                    UiAction::Refresh,
                    cx,
                )
                .tooltip("Refresh usage"),
            )
            .child(
                self.button(
                    "header-compact",
                    if narrow { "" } else { "Quick view" },
                    Some("overview"),
                    false,
                    UiAction::Compact,
                    cx,
                )
                .tooltip("Quick view"),
            )
    }
    fn page_body(&self, _cx: &Context<Self>) -> Div {
        div()
            .w_full()
            .max_w(px(1200.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap(px(16.))
    }
    fn periods(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(7.))
            .bg(rgb(theme.panel))
            .border_1()
            .border_color(rgb(theme.line))
            .p(px(3.))
            .children(Period::ALL.into_iter().map(|period| {
                self.button(
                    format!("period-{}", period.key()),
                    match period {
                        Period::TwoHours => "2 hours",
                        Period::Day => "24 hours",
                        _ => period.label(),
                    },
                    None,
                    self.period == period,
                    UiAction::Period(period),
                    cx,
                )
            }))
    }
    fn metric(&self, name: &str, value: String, note: &str, dense: bool) -> Div {
        let theme = self.theme();
        div()
            .flex_1()
            .min_w_0()
            .child(label(name.to_owned(), theme).text_size(px(12.)))
            .child(
                div()
                    .font_family(display_font())
                    .text_size(px(if dense { 23. } else { 28. }))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(value),
            )
            .when(!note.is_empty(), |metric| {
                metric.child(label(note.to_owned(), theme).text_size(px(11.)).truncate())
            })
    }
    fn badge(&self, text: &str, active: bool) -> Div {
        let theme = self.theme();
        div()
            .rounded(px(4.))
            .px(px(7.))
            .py(px(3.))
            .text_size(px(11.))
            .text_color(rgb(if active { theme.ink } else { theme.muted }))
            .bg(rgb(if active { theme.wash } else { theme.bg }))
            .child(text.to_owned())
    }
    pub(super) fn overview(&self, width: f32, height: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let analysis = analytics::analyze(self.data(), &self.config, self.period, None, Utc::now());
        let accounts = self.visible_accounts();
        let layout = layout::OverviewLayout::new(
            width,
            height,
            accounts.len(),
            accounts
                .iter()
                .map(|status| status.limits.len())
                .max()
                .unwrap_or(0),
        );
        let compact_periods = layout.width < 650.;
        let periods =
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .children(Period::ALL.into_iter().map(|period| {
                    self.button(
                        format!("overview-period-{}", period.key()),
                        if compact_periods {
                            period.key()
                        } else {
                            match period {
                                Period::TwoHours => "2 hours",
                                Period::Day => "24 hours",
                                _ => period.label(),
                            }
                        },
                        None,
                        self.period == period,
                        UiAction::Period(period),
                        cx,
                    )
                    .tooltip(period.label())
                }));
        let body = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(layout.gap))
            .child(
                div()
                    .h(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(periods)
                    .child(div().flex_1())
                    .child(
                        self.button(
                            "overview-connections",
                            if compact_periods { "" } else { "Connections" },
                            Some("connections"),
                            false,
                            UiAction::Page(Page::Connections),
                            cx,
                        )
                        .tooltip("Manage accounts"),
                    ),
            );
        if accounts.is_empty() {
            return body.child(
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(title("No accounts found", theme))
                        .child(label("Open an agent or add its data folder.", theme))
                        .child(self.button(
                            "empty-connect",
                            "Add account",
                            Some("connections"),
                            false,
                            UiAction::Page(Page::Connections),
                            cx,
                        )),
                ),
            );
        }
        let mut cost_note = if layout.width >= 800.
            && analysis.totals.reported_rows > 0
            && analysis.totals.estimated_rows > 0
            && analysis.totals.events
                == analysis.totals.reported_rows + analysis.totals.estimated_rows
        {
            format!(
                "{} reported + {} estimated",
                self.money(analysis.totals.reported),
                self.money(analysis.totals.estimated)
            )
        } else {
            self.cost_caption(&analysis.totals)
        };
        if layout.width < 650. {
            cost_note = cost_note
                .replace("Estimated · API rates", "Estimated")
                .replace("reported + estimated", "mixed")
                .replace("Reported + estimated", "Reported + est.");
        }
        let summary = div()
            .h(px(layout.summary_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(if layout.dense { 12. } else { 24. }))
            .child(self.metric(
                "Tokens",
                self.number(analysis.totals.tokens.total()),
                "",
                layout.dense,
            ))
            .child(div().w(px(1.)).h_full().bg(rgb(theme.line)))
            .child(self.metric(
                "Usage cost",
                self.spend(&analysis.totals),
                &cost_note,
                layout.dense,
            ))
            .child(div().w(px(1.)).h_full().bg(rgb(theme.line)))
            .child(self.metric(
                "Sessions",
                self.number(analysis.totals.sessions as u64),
                "",
                layout.dense,
            ));
        let heading = div()
            .h(px(20.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(label("Current allowances", theme).text_size(px(12.)))
            .child(div().flex_1())
            .child(label(format!("{} accounts", accounts.len()), theme).text_size(px(11.)))
            .child(
                self.button(
                    "all-limits",
                    "All limits",
                    None,
                    false,
                    UiAction::Page(Page::Limits),
                    cx,
                )
                .h(px(20.)),
            );
        let grid = div()
            .id("home-accounts")
            .h(px(layout.accounts_height))
            .flex_shrink_0()
            .when(layout.scroll_accounts, |grid| grid.overflow_y_scroll())
            .child(div().flex().flex_col().gap(px(layout.gap)).children(
                accounts.chunks(layout.columns).map(|row| {
                    div()
                        .flex()
                        .gap(px(layout.gap))
                        .children(row.iter().map(|status| {
                            self.account_card(
                                status,
                                &analysis.providers[status.provider.index()],
                                layout,
                                cx,
                            )
                        }))
                }),
            ));
        body.child(summary)
            .child(heading)
            .child(grid)
            .child(self.chart(&analysis, layout.chart_height, layout.dense, cx))
            .child(
                div()
                    .h(px(20.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(label("USD · Subscription fees excluded", theme).text_size(px(11.)))
                    .child(div().flex_1())
                    .child(
                        self.button(
                            "account-export",
                            if compact_periods { "" } else { "Export CSV" },
                            Some("export"),
                            false,
                            UiAction::Export,
                            cx,
                        )
                        .h(px(20.))
                        .tooltip("Export CSV"),
                    ),
            )
    }

    fn account_card(
        &self,
        status: &ProviderStatus,
        totals: &Totals,
        layout: layout::OverviewLayout,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme();
        let provider = status.provider;
        let has_history = status.sources > 0
            || (provider == Provider::Cursor && status.connected)
            || self.data().records.iter().any(|r| r.provider == provider);
        let tokens = if has_history {
            self.number(totals.tokens.total())
        } else {
            "—".into()
        };
        let reading = if !status.stale && status.checked_at.is_some() {
            "Live"
        } else if status.checked_at.is_some() {
            "Cached"
        } else if status.connected {
            "Local"
        } else {
            "Detected"
        };
        let plan = status.plan.as_deref().map(plan_name).unwrap_or_default();
        let extras = status.limits.len().saturating_sub(layout.limits);
        let title_text = if layout.cards && provider == Provider::Codex {
            "ChatGPT / Codex"
        } else {
            provider.account_name()
        };
        let subtitle = [plan.as_str(), reading]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let header_hint = format!("{}\n{}", subtitle, self.cost_caption(totals));
        let header = div()
            .h(px(if layout.cards { 42. } else { 24. }))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                provider_mark(provider, self.config.dark).size(px(if layout.cards {
                    28.
                } else {
                    22.
                })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .when(!layout.cards, |column| column.line_height(relative(1.1)))
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "account-title-{}",
                                provider.key()
                            )))
                            .cursor_pointer()
                            .text_size(px(if layout.cards { 14. } else { 12. }))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.perform(UiAction::AccountActivity(provider), window, cx)
                            }))
                            .tooltip(move |window, cx| {
                                gpui_component::tooltip::Tooltip::new(if extras > 0 {
                                    format!(
                                        "{header_hint}\n{extras} additional allowances in Limits"
                                    )
                                } else {
                                    header_hint.clone()
                                })
                                .build(window, cx)
                            })
                            .child(if extras > 0 {
                                format!("{title_text} +{extras}")
                            } else {
                                title_text.into()
                            }),
                    )
                    .child(
                        label(subtitle.clone(), theme)
                            .text_size(px(if layout.cards { 11. } else { 9. }))
                            .truncate(),
                    ),
            )
            .when(!layout.cards, |row| {
                row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .w(px(54.))
                                .text_align(TextAlign::Right)
                                .line_height(relative(1.))
                                .child(label("Tokens", theme).text_size(px(9.)))
                                .child(
                                    div()
                                        .font_family(mono_font())
                                        .text_size(px(11.))
                                        .child(tokens.clone()),
                                ),
                        )
                        .child(
                            div()
                                .w(px(72.))
                                .text_align(TextAlign::Right)
                                .line_height(relative(1.))
                                .child(
                                    label(
                                        if totals.estimated_rows > 0 {
                                            if totals.reported_rows > 0 {
                                                "Mixed cost"
                                            } else {
                                                "Est. cost"
                                            }
                                        } else {
                                            "Usage cost"
                                        },
                                        theme,
                                    )
                                    .text_size(px(9.)),
                                )
                                .child(
                                    div()
                                        .font_family(mono_font())
                                        .text_size(px(11.))
                                        .child(self.spend(totals)),
                                ),
                        ),
                )
            })
            .when(layout.cards, |row| {
                row.child(
                    self.button(
                        format!("account-open-{}", provider.key()),
                        "",
                        Some("arrow"),
                        false,
                        UiAction::AccountActivity(provider),
                        cx,
                    )
                    .tooltip("View account activity"),
                )
            });
        let allowances = div()
            .flex()
            .gap(px(if layout.cards { 0. } else { 12. }))
            .when(layout.cards, |row| row.flex_col())
            .when(status.limits.is_empty(), |row| {
                row.items_center().child(
                    label(
                        if status.detected && !status.connected {
                            "Sign in to see allowances"
                        } else {
                            "No allowance reported"
                        },
                        theme,
                    )
                    .text_size(px(12.)),
                )
            })
            .children(status.limits.iter().take(layout.limits).map(|limit| {
                div()
                    .when(!layout.cards, |cell| cell.flex_1().min_w_0())
                    .when(layout.cards, |cell| cell.h(px(48.)).flex_shrink_0())
                    .child(self.account_quota(limit, status.stale, provider, true))
            }));
        panel(theme)
            .w(px(layout.item_width()))
            .h(px(layout.item_height))
            .flex_shrink_0()
            .p(px(if layout.cards { 16. } else { 6. }))
            .flex()
            .flex_col()
            .child(header)
            .child(
                allowances
                    .flex_1()
                    .min_h_0()
                    .when(!layout.cards, |row| row.mt(px(4.))),
            )
            .when(layout.cards, |card| {
                card.child(
                    div()
                        .h(px(48.))
                        .flex_shrink_0()
                        .pt(px(7.))
                        .border_t_1()
                        .border_color(rgb(theme.line))
                        .flex()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(label("Tokens", theme).text_size(px(11.)))
                                .child(
                                    div()
                                        .font_family(mono_font())
                                        .text_size(px(17.))
                                        .child(tokens.clone()),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(label("Usage cost", theme).text_size(px(11.)))
                                .child(
                                    div()
                                        .font_family(mono_font())
                                        .text_size(px(17.))
                                        .child(self.spend(totals)),
                                )
                                .child(
                                    label(self.cost_caption(totals), theme)
                                        .text_size(px(10.))
                                        .truncate(),
                                ),
                        ),
                )
            })
    }

    fn account_quota(
        &self,
        limit: &LimitWindow,
        stale: bool,
        provider: Provider,
        dense: bool,
    ) -> Div {
        let theme = self.theme();
        let expired = limit.expired(Utc::now());
        let warning = !expired && !stale && limit.remaining() <= 25.;
        let bar_color = if expired || stale {
            theme.muted
        } else if limit.remaining() <= 10. {
            0xe03e3e
        } else if warning {
            0xcb912f
        } else {
            color(provider, self.config.dark)
        };
        let remaining = if self.config.privacy {
            "Hidden".into()
        } else if expired {
            "Refresh needed".into()
        } else if let Some(credits) = &limit.credits {
            format!("{:.2} left", (credits.limit - credits.used).max(0.))
        } else {
            format!("{:.0}% left", limit.remaining())
        };
        let reset = if self.config.privacy {
            "Reset hidden".into()
        } else {
            reset_text(limit.resets_at)
        };
        div()
            .h(px(if dense { 38. } else { 56. }))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(if dense { 2. } else { 4. }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        label(limit.label.clone(), theme)
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(if warning && !self.config.privacy {
                                bar_color
                            } else {
                                theme.ink
                            }))
                            .child(remaining),
                    ),
            )
            .child(
                div()
                    .h(px(4.))
                    .w_full()
                    .rounded_full()
                    .bg(rgb(theme.line))
                    .when(!self.config.privacy && !expired, |bar| {
                        bar.child(
                            div()
                                .h_full()
                                .w(relative(limit.remaining() as f32 / 100.))
                                .rounded_full()
                                .bg(rgb(bar_color)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        label(reset, theme)
                            .text_size(px(10.))
                            .truncate()
                            .flex_1()
                            .min_w_0(),
                    )
                    .when(warning && !self.config.privacy, |row| {
                        row.child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(bar_color))
                                .child("Low allowance"),
                        )
                    }),
            )
    }
    fn quota(&self, limit: &LimitWindow, stale: bool, provider: Provider) -> Div {
        let theme = self.theme();
        let expired = limit.expired(Utc::now());
        let remaining = if self.config.privacy {
            "Hidden".into()
        } else if expired {
            "Refresh needed".into()
        } else if let Some(credits) = &limit.credits {
            format!("{:.2} credits left", (credits.limit - credits.used).max(0.))
        } else {
            format!("{:.0}% left", limit.remaining())
        };
        let bar_color = if expired || stale {
            theme.muted
        } else if limit.remaining() <= 10.0 {
            0xe03e3e
        } else if limit.remaining() <= 25.0 {
            0xcb912f
        } else {
            color(provider, self.config.dark)
        };
        div()
            .flex()
            .flex_col()
            .line_height(relative(1.2))
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(label(limit.label.clone(), theme).text_size(px(12.)))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(remaining),
                    ),
            )
            .child(
                div()
                    .h(px(5.))
                    .w_full()
                    .rounded_full()
                    .bg(rgb(theme.line))
                    .overflow_hidden()
                    .when(!self.config.privacy && !expired, |bar| {
                        bar.child(
                            div()
                                .h_full()
                                .w(relative(limit.remaining() as f32 / 100.))
                                .bg(rgb(bar_color))
                                .rounded_full(),
                        )
                    }),
            )
            .child(
                label(
                    if self.config.privacy {
                        "Reset hidden".into()
                    } else {
                        format!(
                            "{}{}",
                            reset_text(limit.resets_at),
                            if stale { " · cached" } else { "" }
                        )
                    },
                    theme,
                )
                .text_size(px(11.)),
            )
    }
    fn chart(&self, analysis: &Analysis, height: f32, dense: bool, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let values = analysis
            .buckets
            .iter()
            .map(|bucket| {
                if self.token_chart {
                    bucket.tokens.map(|count| count as f64)
                } else {
                    bucket.cost
                }
            })
            .collect::<Vec<_>>();
        let maximum = values
            .iter()
            .map(|value| value.iter().sum::<f64>())
            .fold(0.0, f64::max);
        let legend = height >= 150.;
        let tiny = height < 100.;
        let padding = if tiny {
            6.
        } else if dense {
            8.
        } else {
            12.
        };
        let header_height = if tiny { 24. } else { 28. };
        let axis_height = if tiny { 14. } else { 18. };
        let plot_height = height
            - padding * 2.
            - 2.
            - header_height
            - axis_height
            - 4.
            - if legend { 20. } else { 0. };
        let caption = self
            .hovered_day
            .and_then(|day| analysis.buckets.get(day))
            .map(|bucket| {
                format!(
                    "{} · {}",
                    self.period.bucket_label(bucket.start),
                    if self.token_chart {
                        format!("{} tokens", self.number(bucket.tokens.iter().sum()))
                    } else if bucket.cost_rows.iter().sum::<usize>() > 0 {
                        self.money(bucket.cost.iter().sum())
                    } else {
                        "No public rate".into()
                    }
                )
            })
            .unwrap_or_else(|| self.period.label().into());
        panel(theme)
            .h(px(height))
            .flex_shrink_0()
            .p(px(padding))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(header_height))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(title("Recent usage", theme).text_size(px(if tiny { 12. } else { 13. })))
                    .when(!dense, |row| {
                        row.child(
                            label(
                                if self.config.privacy {
                                    "Hidden".into()
                                } else {
                                    caption
                                },
                                theme,
                            )
                            .text_size(px(11.))
                            .truncate()
                            .flex_1()
                            .min_w_0(),
                        )
                    })
                    .when(dense, |row| row.child(div().flex_1()))
                    .child(self.button(
                        "chart-tokens",
                        "Tokens",
                        None,
                        self.token_chart,
                        UiAction::Metric(true),
                        cx,
                    ))
                    .child(self.button(
                        "chart-cost",
                        "Cost",
                        None,
                        !self.token_chart,
                        UiAction::Metric(false),
                        cx,
                    )),
            )
            .child(
                div()
                    .id("daily-plot")
                    .mt(px(4.))
                    .h(px(plot_height))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .items_end()
                    .gap(px(if analysis.buckets.len() > 30 { 2. } else { 4. }))
                    .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                        if !hovered && view.hovered_day.take().is_some() {
                            cx.notify();
                        }
                    }))
                    .when(self.config.privacy || maximum == 0., |plot| {
                        plot.items_center().justify_center().child(
                            label(
                                if self.config.privacy {
                                    "Values hidden"
                                } else if self.token_chart {
                                    "No token usage"
                                } else if analysis.totals.spend().is_some() {
                                    "Cost is $0.00"
                                } else {
                                    "No priced usage"
                                },
                                theme,
                            )
                            .text_size(px(11.)),
                        )
                    })
                    .when(!self.config.privacy && maximum > 0., |plot| {
                        plot.children(values.iter().enumerate().map(|(index, values)| {
                            let bucket = &analysis.buckets[index];
                            let mut details = format!(
                                "{}–{}",
                                self.period.bucket_label(bucket.start),
                                self.period.bucket_label(bucket.end)
                            );
                            for status in self.visible_accounts() {
                                let provider = status.provider;
                                let value = if self.token_chart {
                                    format!(
                                        "{} tokens",
                                        self.number(bucket.tokens[provider.index()])
                                    )
                                } else if bucket.cost_rows[provider.index()] > 0 {
                                    self.money(bucket.cost[provider.index()])
                                } else {
                                    "—".into()
                                };
                                details.push_str(&format!("\n{}: {value}", provider.name()));
                            }
                            div()
                                .id(SharedString::from(format!("day-{index}")))
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .justify_end()
                                .gap(px(1.))
                                .opacity(if self.hovered_day.is_some_and(|day| day != index) {
                                    0.45
                                } else {
                                    1.
                                })
                                .on_mouse_move(cx.listener(move |view, _, _, cx| {
                                    if view.hovered_day != Some(index) {
                                        view.hovered_day = Some(index);
                                        cx.notify();
                                    }
                                }))
                                .tooltip(move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(details.clone())
                                        .build(window, cx)
                                })
                                .children(Provider::ALL.into_iter().rev().map(|provider| {
                                    div()
                                        .h(px((values[provider.index()] / maximum
                                            * (plot_height - Provider::ALL.len() as f32).max(0.)
                                                as f64)
                                            as f32))
                                        .w_full()
                                        .rounded(px(2.))
                                        .bg(rgb(color(provider, self.config.dark)))
                                }))
                        }))
                    }),
            )
            .child(
                div()
                    .h(px(axis_height))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        label(
                            analysis
                                .buckets
                                .first()
                                .map(|bucket| self.period.bucket_label(bucket.start))
                                .unwrap_or_default(),
                            theme,
                        )
                        .text_size(px(10.)),
                    )
                    .child(
                        label(
                            analysis
                                .buckets
                                .last()
                                .map(|bucket| self.period.bucket_label(bucket.end))
                                .unwrap_or_default(),
                            theme,
                        )
                        .text_size(px(10.)),
                    ),
            )
            .when(legend, |panel| {
                panel.child(
                    div()
                        .h(px(20.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .children(self.visible_accounts().iter().map(|status| {
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .child(dot(status.provider, self.config.dark))
                                .child(
                                    label(status.provider.name(), theme)
                                        .text_size(px(11.))
                                        .truncate(),
                                )
                        })),
                )
            })
    }
    fn table_filters(&self, table: Table, width: f32, cx: &Context<Self>) -> Div {
        use gpui_component::{
            Sizable,
            input::Input,
            menu::{DropdownMenu, PopupMenuItem},
        };
        let query = match table {
            Table::Models => &self.model_query,
            Table::Activity => &self.activity_query,
        };
        let cost_filter = match table {
            Table::Models => self.model_cost,
            Table::Activity => self.activity_cost,
        };
        let weak = cx.weak_entity();
        let provider = self.provider;
        let options = std::iter::once(None)
            .chain(self.visible_accounts().iter().map(|s| Some(s.provider)))
            .collect::<Vec<_>>();
        let account = self
            .button(
                format!("account-filter-{table:?}"),
                format!(
                    "{} ▾",
                    provider.map(Provider::name).unwrap_or("All accounts")
                ),
                None,
                provider.is_some(),
                UiAction::Noop,
                cx,
            )
            .dropdown_menu(move |mut menu, _, _| {
                for option in &options {
                    let option = *option;
                    let view = weak.clone();
                    menu = menu.item(
                        PopupMenuItem::new(option.map(Provider::name).unwrap_or("All accounts"))
                            .checked(provider == option)
                            .on_click(move |_, window, cx| {
                                let _ = view.update(cx, |view, cx| {
                                    view.perform(UiAction::Provider(option), window, cx)
                                });
                            }),
                    );
                }
                menu
            });
        let weak = cx.weak_entity();
        let costs = self
            .button(
                format!("cost-filter-{table:?}"),
                format!("{} ▾", cost_filter.label()),
                None,
                cost_filter != CostFilter::All,
                UiAction::Noop,
                cx,
            )
            .dropdown_menu(move |mut menu, _, _| {
                for option in CostFilter::ALL {
                    let view = weak.clone();
                    menu = menu.item(
                        PopupMenuItem::new(option.label())
                            .checked(cost_filter == option)
                            .on_click(move |_, window, cx| {
                                let _ = view.update(cx, |view, cx| {
                                    view.perform(UiAction::CostFilter(table, option), window, cx)
                                });
                            }),
                    );
                }
                menu
            });
        let active = provider.is_some()
            || cost_filter != CostFilter::All
            || !query.read(cx).value().is_empty();
        let controls = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(account)
            .child(costs)
            .when(active, |row| {
                row.child(self.button(
                    format!("clear-filters-{table:?}"),
                    "Clear",
                    None,
                    false,
                    UiAction::ClearFilters(table),
                    cx,
                ))
            });
        let search = Input::new(query)
            .small()
            .h(px(30.))
            .cleanable(true)
            .prefix(gpui_component::IconName::Search)
            .flex_1()
            .min_w_0();
        let body = div().flex().flex_col().gap(px(8.)).flex_shrink_0();
        if width >= 650. {
            body.child(controls).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(search)
                    .child(self.periods(cx)),
            )
        } else {
            body.child(search).child(controls).child(self.periods(cx))
        }
    }

    fn sort_header(
        &self,
        table: Table,
        column: Column,
        width: Option<f32>,
        numeric: bool,
        cx: &Context<Self>,
    ) -> gpui_component::button::Button {
        use gpui_component::{
            Sizable,
            button::{Button, ButtonCustomVariant, ButtonVariants},
        };
        let theme = self.theme();
        let sort = match table {
            Table::Models => self.model_sort,
            Table::Activity => self.activity_sort,
        };
        Button::new(SharedString::from(format!("sort-{table:?}-{column:?}")))
            .small()
            .h(px(34.))
            .px(px(0.))
            .rounded(px(3.))
            .label(format!("{}{}", column.label(), sort.marker(column)))
            .text_size(px(11.))
            .justify_start()
            .when(numeric, |button| button.justify_end())
            .when_some(width, |button, width| button.w(px(width)).flex_shrink_0())
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(rgb(theme.bg).into())
                    .foreground(
                        rgb(if sort.column == column {
                            theme.ink
                        } else {
                            theme.muted
                        })
                        .into(),
                    )
                    .border(rgb(theme.bg).into())
                    .hover(rgb(theme.wash).into())
                    .active(rgb(theme.wash).into()),
            )
            .tooltip(format!("Sort by {}", column.label().to_lowercase()))
            .on_click(cx.listener(move |view, _, window, cx| {
                view.perform(UiAction::Sort(table, column), window, cx)
            }))
    }

    pub(super) fn models_view(&self, width: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let content_width =
            width - layout::sidebar_width(width) - layout::page_padding(width, 860.) * 2.;
        let mut analysis = analytics::analyze(
            self.data(),
            &self.config,
            self.period,
            self.provider,
            Utc::now(),
        );
        let total = analysis.models.len();
        let query = self.model_query.read(cx).value();
        analysis.models.retain(|row| {
            self.model_cost.totals(&row.totals)
                && table::matches_query(
                    &query,
                    &[&row.model, row.provider.name(), row.provider.key()],
                )
        });
        self.model_sort.models(&mut analysis.models);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.table_filters(Table::Models, content_width, cx))
            .child(
                panel(theme)
                    .p(px(0.))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(48.))
                            .flex_shrink_0()
                            .px(px(16.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(title("Model usage", theme).text_size(px(14.)))
                            .child(div().flex_1())
                            .child(
                                label(
                                    if analysis.models.len() == total {
                                        format!("{total} models")
                                    } else {
                                        format!("{} of {total} models", analysis.models.len())
                                    },
                                    theme,
                                )
                                .text_size(px(12.)),
                            ),
                    )
                    .child(self.model_header(content_width, cx))
                    .child(
                        div()
                            .id("model-table-rows")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .when(analysis.models.is_empty(), |rows| {
                                rows.child(div().p(px(24.)).child(label(
                                    if total == 0 {
                                        "No model usage in this period"
                                    } else {
                                        "No models match these filters"
                                    },
                                    theme,
                                )))
                            })
                            .children(
                                analysis
                                    .models
                                    .iter()
                                    .map(|row| self.model_row(row, content_width)),
                            ),
                    ),
            )
    }
    fn model_header(&self, width: f32, cx: &Context<Self>) -> Div {
        let narrow = width < 650.;
        let expanded = width >= 1000.;
        let theme = self.theme();
        div()
            .h(px(36.))
            .flex_shrink_0()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .bg(rgb(theme.bg))
            .border_t_1()
            .border_color(rgb(theme.line))
            .child(
                self.sort_header(Table::Models, Column::Model, None, false, cx)
                    .flex_1()
                    .min_w_0(),
            )
            .when(!narrow, |row| {
                row.child(self.sort_header(Table::Models, Column::Account, Some(90.), false, cx))
            })
            .child(self.sort_header(
                Table::Models,
                Column::Tokens,
                Some(if narrow { 72. } else { 88. }),
                true,
                cx,
            ))
            .when(expanded, |row| {
                row.child(self.sort_header(Table::Models, Column::Input, Some(76.), true, cx))
                    .child(self.sort_header(Table::Models, Column::Output, Some(76.), true, cx))
            })
            .when(!narrow, |row| {
                row.child(self.sort_header(Table::Models, Column::CacheRead, Some(88.), true, cx))
            })
            .when(expanded, |row| {
                row.child(self.sort_header(Table::Models, Column::CacheWrite, Some(88.), true, cx))
                    .child(self.sort_header(Table::Models, Column::Sessions, Some(72.), true, cx))
            })
            .child(self.sort_header(
                Table::Models,
                Column::Cost,
                Some(if narrow { 88. } else { 110. }),
                true,
                cx,
            ))
    }
    fn model_row(&self, row: &analytics::ModelRow, width: f32) -> Div {
        let theme = self.theme();
        let narrow = width < 650.;
        let expanded = width >= 1000.;
        let totals = &row.totals;
        div()
            .h(px(54.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.table_cell(&row.model, None, false, false))
                    .when(narrow, |cell| {
                        cell.child(label(row.provider.name(), theme).text_size(px(10.)))
                    }),
            )
            .when(!narrow, |line| {
                line.child(
                    div()
                        .w(px(90.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child(dot(row.provider, self.config.dark))
                        .child(self.table_cell(row.provider.name(), None, false, false)),
                )
            })
            .child(self.table_cell(
                &self.number(totals.tokens.total()),
                Some(if narrow { 72. } else { 88. }),
                true,
                false,
            ))
            .when(expanded, |line| {
                line.child(self.table_cell(
                    &self.number(totals.tokens.input),
                    Some(76.),
                    true,
                    false,
                ))
                .child(self.table_cell(
                    &self.number(totals.tokens.output),
                    Some(76.),
                    true,
                    false,
                ))
            })
            .when(!narrow, |line| {
                line.child(self.table_cell(
                    &self.number(totals.tokens.cache_read),
                    Some(88.),
                    true,
                    false,
                ))
            })
            .when(expanded, |line| {
                line.child(self.table_cell(
                    &self.number(totals.tokens.cache_write),
                    Some(88.),
                    true,
                    false,
                ))
                .child(self.table_cell(
                    &self.number(totals.sessions as u64),
                    Some(72.),
                    true,
                    false,
                ))
            })
            .child(
                div()
                    .w(px(if narrow { 88. } else { 110. }))
                    .flex_shrink_0()
                    .text_align(TextAlign::Right)
                    .child(self.table_cell(&self.spend(totals), None, true, false))
                    .child(
                        label(self.cost_caption(totals).replace(" · API rates", ""), theme)
                            .text_size(px(10.))
                            .truncate(),
                    ),
            )
    }
    fn table_cell(&self, text: &str, width: Option<f32>, numeric: bool, heading: bool) -> Div {
        let theme = self.theme();
        div()
            .when_some(width, |cell, width| cell.w(px(width)).flex_shrink_0())
            .text_size(px(if heading { 12. } else { 13. }))
            .truncate()
            .text_color(rgb(if heading { theme.muted } else { theme.ink }))
            .when(numeric, |cell| {
                cell.text_align(TextAlign::Right)
                    .when(!heading, |cell| cell.font_family(mono_font()))
            })
            .child(text.to_owned())
    }
    pub(super) fn limits_view(&self, width: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let accounts = self.visible_accounts();
        let content_width =
            width - layout::sidebar_width(width) - layout::page_padding(width, 860.) * 2.;
        let columns = ((content_width - 32.) / 240.).floor().max(1.) as usize;
        self.page_body(cx)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(label("Subscription allowances", theme))
                    .child(self.button(
                        "limits-connect",
                        "Connections",
                        None,
                        false,
                        UiAction::Page(Page::Connections),
                        cx,
                    )),
            )
            .when(accounts.is_empty(), |body| {
                body.child(panel(theme).child(label("No connected accounts", theme)))
            })
            .children(accounts.iter().map(|status| {
                panel(theme)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(provider_mark(status.provider, self.config.dark))
                            .child(
                                div()
                                    .flex_1()
                                    .child(title(status.provider.account_name(), theme))
                                    .child(
                                        label(
                                            status
                                                .plan
                                                .as_deref()
                                                .map(plan_name)
                                                .unwrap_or_else(|| "Plan not reported".into()),
                                            theme,
                                        )
                                        .mt(px(3.)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(20.))
                            .flex()
                            .gap(px(24.))
                            .when(status.limits.is_empty(), |row| {
                                row.child(label(
                                    if self.config.online {
                                        "No account allowances reported"
                                    } else {
                                        "Account checks are paused"
                                    },
                                    theme,
                                ))
                            })
                            .flex_col()
                            .children(status.limits.chunks(columns).map(|limits| {
                                div()
                                    .flex()
                                    .gap(px(24.))
                                    .children(limits.iter().map(|limit| {
                                        div().flex_1().min_w_0().child(self.quota(
                                            limit,
                                            status.stale,
                                            status.provider,
                                        ))
                                    }))
                            })),
                    )
                    .child(
                        label(
                            status
                                .checked_at
                                .map(|at| {
                                    format!(
                                        "Checked {}",
                                        at.with_timezone(&Local).format("%b %-d, %H:%M")
                                    )
                                })
                                .unwrap_or_else(|| "Not checked".into()),
                            theme,
                        )
                        .text_size(px(11.))
                        .mt(px(16.)),
                    )
            }))
    }
    pub(super) fn activity_records<'a>(&'a self, cx: &App) -> Vec<&'a UsageRecord> {
        let query = self.activity_query.read(cx).value();
        let now = Utc::now();
        let mut records = self
            .data()
            .records
            .iter()
            .filter(|record| {
                self.config.enabled[record.provider.index()]
                    && analytics::matches(record, self.period, self.provider, now)
                    && self.activity_cost.record(record)
                    && table::matches_query(
                        &query,
                        &[
                            &record.model,
                            record.provider.name(),
                            record.provider.key(),
                            if self.config.privacy {
                                ""
                            } else {
                                &record.project
                            },
                        ],
                    )
            })
            .collect::<Vec<_>>();
        self.activity_sort.activity(&mut records);
        records
    }
    pub(super) fn activity(&self, width: f32, viewport_height: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let content_width = width
            - layout::sidebar_width(width)
            - layout::page_padding(width, viewport_height) * 2.;
        let now = Utc::now();
        let total = self
            .data()
            .records
            .iter()
            .filter(|record| {
                self.config.enabled[record.provider.index()]
                    && analytics::matches(record, self.period, self.provider, now)
            })
            .count();
        let records = self.activity_records(cx);
        let detail = self
            .detail
            .as_ref()
            .and_then(|id| records.iter().find(|record| record.id == *id));
        let fitted = rows_that_fit(
            width,
            viewport_height,
            detail.is_some(),
            self.config.privacy,
        );
        let page_size = self.activity_rows.unwrap_or(fitted).max(1);
        let page_count = records.len().div_ceil(page_size);
        let page = self.activity_page.min(page_count.saturating_sub(1));
        let start = page * page_size;
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.table_filters(Table::Activity, content_width, cx))
            .when_some(detail, |body, record| {
                body.child(
                    div()
                        .id("activity-detail")
                        .h(px(activity_detail_height(width, viewport_height)))
                        .flex_shrink_0()
                        .overflow_y_scroll()
                        .child(self.usage_detail(record, content_width, cx)),
                )
            })
            .child(
                panel(theme)
                    .p(px(0.))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(48.))
                            .flex_shrink_0()
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(title("Usage events", theme).text_size(px(14.)))
                            .when(content_width >= 650., |row| {
                                row.child(
                                    label(
                                        if total == records.len() {
                                            format!("{total} events")
                                        } else {
                                            format!("{} of {total} events", records.len())
                                        },
                                        theme,
                                    )
                                    .text_size(px(11.)),
                                )
                            })
                            .child(div().flex_1())
                            .child(self.activity_row_options(cx))
                            .child(
                                self.button(
                                    "activity-export",
                                    if content_width >= 650. {
                                        "Export CSV"
                                    } else {
                                        ""
                                    },
                                    Some("export"),
                                    false,
                                    UiAction::Export,
                                    cx,
                                )
                                .tooltip("Export CSV"),
                            ),
                    )
                    .child(self.activity_header(content_width, cx))
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "activity-rows-{page}-{page_size}"
                            )))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .when(records.is_empty(), |rows| {
                                rows.child(div().p(px(24.)).child(label(
                                    if total == 0 {
                                        "No usage events in this period"
                                    } else {
                                        "No events match these filters"
                                    },
                                    theme,
                                )))
                            })
                            .children(records.iter().skip(start).take(page_size).map(|record| {
                                let id = record.id.clone();
                                let selected = self.detail.as_deref() == Some(id.as_str());
                                div()
                                    .id(SharedString::from(format!("record-{id}")))
                                    .cursor_pointer()
                                    .bg(rgb(if selected { theme.wash } else { theme.panel }))
                                    .hover(move |style| style.bg(rgb(theme.wash)))
                                    .child(self.activity_columns(record, content_width))
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.perform(UiAction::Detail(id.clone()), window, cx)
                                    }))
                            })),
                    )
                    .child(
                        self.activity_pager(
                            page,
                            page_count,
                            page_size,
                            records.len(),
                            content_width,
                            cx,
                        )
                        .flex_shrink_0(),
                    ),
            )
    }
    fn activity_row_options(&self, cx: &Context<Self>) -> impl IntoElement {
        use gpui_component::menu::{DropdownMenu, PopupMenuItem};
        let weak = cx.weak_entity();
        let selected = self.activity_rows;
        self.button(
            "activity-row-count",
            format!(
                "Rows: {} ▾",
                selected.map(|n| n.to_string()).unwrap_or("Fit".into())
            ),
            None,
            false,
            UiAction::Noop,
            cx,
        )
        .dropdown_menu(move |mut menu, _, _| {
            for (limit, name) in [
                (None, "Fit window"),
                (Some(25), "25 rows"),
                (Some(50), "50 rows"),
                (Some(100), "100 rows"),
            ] {
                let view = weak.clone();
                menu = menu.item(
                    PopupMenuItem::new(name)
                        .checked(selected == limit)
                        .on_click(move |_, window, cx| {
                            let _ = view.update(cx, |view, cx| {
                                view.perform(UiAction::ActivityRows(limit), window, cx)
                            });
                        }),
                );
            }
            menu
        })
    }
    fn activity_pager(
        &self,
        page: usize,
        page_count: usize,
        page_size: usize,
        total: usize,
        width: f32,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme();
        let start = page * page_size;
        let from = if total == 0 { 0 } else { start + 1 };
        let to = (start + page_size).min(total);
        div()
            .px(px(16.))
            .py(px(12.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(label(format!("{from}–{to} of {}", grouped(total)), theme).text_size(px(12.)))
            .child(div().flex_1())
            .when(page_count > 1, |row| {
                row.child(self.page_step(
                    "activity-prev",
                    "Prev",
                    page > 0,
                    page.saturating_sub(1),
                    cx,
                ))
                .children(
                    (if width < 650. {
                        Vec::new()
                    } else {
                        activity_pages(page, page_count)
                    })
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| match item {
                        Some(number) => self
                            .page_step(
                                format!("activity-page-{number}"),
                                (number + 1).to_string(),
                                true,
                                number,
                                cx,
                            )
                            .bg(rgb(if number == page {
                                theme.wash
                            } else {
                                theme.panel
                            }))
                            .text_color(rgb(if number == page {
                                theme.ink
                            } else {
                                theme.muted
                            }))
                            .font_weight(if number == page {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            }),
                        None => div()
                            .id(SharedString::from(format!("activity-gap-{index}")))
                            .px(px(4.))
                            .text_size(px(12.))
                            .text_color(rgb(theme.muted))
                            .child("…"),
                    }),
                )
                .child(self.page_step(
                    "activity-next",
                    "Next",
                    page + 1 < page_count,
                    page + 1,
                    cx,
                ))
            })
    }
    fn page_step(
        &self,
        id: impl Into<String>,
        label: impl Into<String>,
        enabled: bool,
        page: usize,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        div()
            .id(SharedString::from(id.into()))
            .h(px(28.))
            .min_w(px(28.))
            .px(px(8.))
            .rounded(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .text_color(rgb(if enabled { theme.ink } else { theme.muted }))
            .opacity(if enabled { 1. } else { 0.4 })
            .when(enabled, |chip| {
                chip.cursor_pointer()
                    .hover(move |style| style.bg(rgb(theme.wash)))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.perform(UiAction::ActivityPage(page), window, cx)
                    }))
            })
            .child(label.into())
    }
    fn activity_header(&self, width: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let narrow = width < 650.;
        let full = width >= 850.;
        div()
            .h(px(36.))
            .flex_shrink_0()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .bg(rgb(theme.bg))
            .border_t_1()
            .border_color(rgb(theme.line))
            .when(!narrow, |row| {
                row.child(self.sort_header(Table::Activity, Column::Time, Some(112.), false, cx))
            })
            .when(full, |row| {
                row.child(self.sort_header(Table::Activity, Column::Account, Some(90.), false, cx))
            })
            .child(
                self.sort_header(Table::Activity, Column::Model, None, false, cx)
                    .flex_1()
                    .min_w_0(),
            )
            .when(full, |row| {
                row.child(self.sort_header(Table::Activity, Column::Project, Some(112.), false, cx))
            })
            .child(self.sort_header(
                Table::Activity,
                Column::Tokens,
                Some(if narrow { 72. } else { 88. }),
                true,
                cx,
            ))
            .child(self.sort_header(
                Table::Activity,
                Column::Cost,
                Some(if narrow { 88. } else { 110. }),
                true,
                cx,
            ))
    }
    fn activity_columns(&self, record: &UsageRecord, width: f32) -> Div {
        let theme = self.theme();
        let narrow = width < 650.;
        let full = width >= 850.;
        let (cost, note) = self.event_cost(record);
        div()
            .h(px(54.))
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .when(!narrow, |row| {
                row.child(
                    self.table_cell(
                        &record
                            .timestamp
                            .with_timezone(&Local)
                            .format("%b %-d  %H:%M")
                            .to_string(),
                        Some(112.),
                        false,
                        false,
                    ),
                )
            })
            .when(full, |row| {
                row.child(
                    div()
                        .w(px(90.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child(dot(record.provider, self.config.dark))
                        .child(self.table_cell(record.provider.name(), None, false, false)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.table_cell(&record.model, None, false, false))
                    .when(!full, |cell| {
                        cell.child(
                            label(
                                if narrow {
                                    format!(
                                        "{} · {}",
                                        record.provider.name(),
                                        record
                                            .timestamp
                                            .with_timezone(&Local)
                                            .format("%b %-d %H:%M")
                                    )
                                } else if self.config.privacy || record.project.is_empty() {
                                    record.provider.name().into()
                                } else {
                                    format!("{} · {}", record.provider.name(), record.project)
                                },
                                theme,
                            )
                            .text_size(px(10.))
                            .truncate(),
                        )
                    }),
            )
            .when(full, |row| {
                row.child(self.table_cell(
                    if self.config.privacy {
                        "Hidden"
                    } else {
                        &record.project
                    },
                    Some(112.),
                    false,
                    false,
                ))
            })
            .child(self.table_cell(
                &self.number(record.tokens.total()),
                Some(if narrow { 72. } else { 88. }),
                true,
                false,
            ))
            .child(
                div()
                    .w(px(if narrow { 88. } else { 110. }))
                    .flex_shrink_0()
                    .text_align(TextAlign::Right)
                    .child(self.table_cell(&cost, None, true, false))
                    .child(
                        label(note.replace(" · API rate", ""), theme)
                            .text_size(px(10.))
                            .truncate(),
                    ),
            )
    }
    fn usage_detail(&self, record: &UsageRecord, width: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let dense = width < 650.;
        panel(theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(title("Usage details", theme))
                    .child(div().flex_1())
                    .child(self.button(
                        "detail-close",
                        "Close",
                        None,
                        dense,
                        UiAction::CloseDetail,
                        cx,
                    )),
            )
            .child(
                div()
                    .mt(px(16.))
                    .flex()
                    .gap(px(18.))
                    .child(self.metric(
                        "Account",
                        record.provider.name().into(),
                        &record.model,
                        dense,
                    ))
                    .child({
                        let (amount, note) = self.event_cost(record);
                        self.metric("Cost", amount, note, dense)
                    }),
            )
            .child(
                div()
                    .mt(px(16.))
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .children(
                        [
                            ("Input", record.tokens.input, "Uncached"),
                            ("Output", record.tokens.output, "Includes reasoning"),
                            ("Cache read", record.tokens.cache_read, "Cached input"),
                            ("Cache write", record.tokens.cache_write, "Stored context"),
                        ]
                        .chunks(if dense { 2 } else { 4 })
                        .map(|metrics| {
                            div().flex().gap(px(16.)).children(metrics.iter().map(
                                |(name, value, note)| {
                                    self.metric(name, self.number(*value), note, dense)
                                },
                            ))
                        }),
                    ),
            )
            .when(!self.config.privacy, |detail| {
                detail
                    .child(
                        label(
                            format!(
                                "Project: {}",
                                if record.project.is_empty() {
                                    "Not reported"
                                } else {
                                    &record.project
                                }
                            ),
                            theme,
                        )
                        .mt(px(16.)),
                    )
                    .child(
                        label(format!("Source: {}", record.source), theme)
                            .text_size(px(11.))
                            .mt(px(4.))
                            .overflow_hidden(),
                    )
            })
    }
    pub(super) fn connections(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div().w_full().max_w(px(720.)).mx_auto().child(
            panel(theme).p(px(0.)).overflow_hidden().children(
                Provider::ALL
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, provider)| {
                        self.data()
                            .providers
                            .get(provider.index())
                            .filter(|status| status.provider == provider)
                            .map(|status| self.connection_account(status, index > 0, cx))
                    }),
            ),
        )
    }
    fn connection_account(
        &self,
        status: &ProviderStatus,
        divider: bool,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme();
        let provider = status.provider;
        let included = self.config.enabled[provider.index()];
        let summary = self.connection_summary(status);
        let sources = self.listed_sources(status);
        div()
            .when(divider, |row| {
                row.border_t_1().border_color(rgb(theme.line))
            })
            .px(px(18.))
            .py(px(14.))
            .flex()
            .items_start()
            .gap(px(12.))
            .child(provider_mark(provider, self.config.dark).mt(px(1.)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(title(provider.account_name(), theme).text_size(px(14.)))
                    .when_some(summary.filter(|_| included), |column, summary| {
                        column.child(label(summary, theme).mt(px(3.)).text_size(px(12.)))
                    })
                    .when(included, |column| {
                        column
                            .children(sources.iter().map(|source| {
                                let remove = source.remove.clone();
                                div()
                                    .mt(px(8.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .font_family(mono_font())
                                            .text_size(px(12.))
                                            .text_color(rgb(theme.muted))
                                            .child(source.text.clone()),
                                    )
                                    .when_some(remove, |row, (id, action)| {
                                        row.child(
                                            self.button(id, "Remove", None, false, action, cx),
                                        )
                                    })
                            }))
                            .child(
                                div()
                                    .mt(px(12.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(self.button(
                                        format!("add-home-{}", provider.key()),
                                        "Add folder",
                                        None,
                                        false,
                                        UiAction::AddHome(provider),
                                        cx,
                                    ))
                                    .when(provider == Provider::Cursor, |row| {
                                        row.child(self.button(
                                            "cursor-csv",
                                            "Import CSV",
                                            None,
                                            false,
                                            UiAction::ImportCursor,
                                            cx,
                                        ))
                                    })
                                    .child(div().flex_1())
                                    .child(self.button(
                                        format!("open-account-{}", provider.key()),
                                        "View",
                                        Some("arrow"),
                                        false,
                                        UiAction::OpenProvider(provider),
                                        cx,
                                    )),
                            )
                    }),
            )
            .child(div().mt(px(2.)).child(self.toggle(
                &format!("include-{}", provider.key()),
                included,
                move |view, on, cx| {
                    view.config.enabled[provider.index()] = on;
                    view.save(cx);
                    view.refresh(cx);
                },
                cx,
            )))
    }
    fn toggle(
        &self,
        id: &str,
        on: bool,
        change: impl Fn(&mut Self, bool, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        let change = std::rc::Rc::new(change);
        let change_key = change.clone();
        let (track, knob) = if on {
            (theme.ink, theme.bg)
        } else if self.config.dark {
            (theme.line, theme.muted)
        } else {
            (theme.wash, theme.panel)
        };
        div()
            .id(SharedString::from(id.to_owned()))
            .focusable()
            .tab_index(0)
            .w(px(40.))
            .h(px(24.))
            .flex_shrink_0()
            .rounded_full()
            .cursor_pointer()
            .flex()
            .items_center()
            .px(px(3.))
            .bg(rgb(track))
            .when(on, |track| track.justify_end())
            .when(!self.config.dark && !on, |track| {
                track.border_1().border_color(rgb(theme.line))
            })
            .focus(move |style| style.border_1().border_color(rgb(theme.accent)))
            .on_click(cx.listener(move |view, _, _, cx| {
                change(view, !on, cx);
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    change_key(view, !on, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .size(px(18.))
                    .rounded_full()
                    .bg(rgb(knob))
                    .when(!on && !self.config.dark, |knob| {
                        knob.border_1().border_color(rgb(theme.line))
                    }),
            )
    }
    fn connection_summary(&self, status: &ProviderStatus) -> Option<String> {
        let mut parts = Vec::new();
        if status.provider == Provider::Cursor && !self.config.cursor_imports.is_empty() {
            parts.push("CSV".into());
        } else if status.provider != Provider::Cursor && status.sources > 0 {
            parts.push(if status.sources == 1 {
                "1 file".into()
            } else {
                format!("{} files", status.sources)
            });
        }
        if let Some(allowance) = self.allowance_label(status) {
            parts.push(allowance);
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" · "))
        }
    }
    fn allowance_label(&self, status: &ProviderStatus) -> Option<String> {
        if !self.config.online {
            return Some(if status.checked_at.is_some() {
                "Cached".into()
            } else {
                "Paused".into()
            });
        }
        if !status.stale && status.checked_at.is_some() {
            return Some(
                status
                    .plan
                    .as_deref()
                    .map(plan_name)
                    .unwrap_or_else(|| "Live".into()),
            );
        }
        if reported_error(&status.detail) {
            return Some(status.detail.clone());
        }
        if status.checked_at.is_some() {
            return Some("Cached".into());
        }
        if status.detected {
            return Some("No sign-in".into());
        }
        None
    }
    fn listed_sources(&self, status: &ProviderStatus) -> Vec<ListedSource> {
        let provider = status.provider;
        let mut listed = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (index, path) in self.config.custom_homes[provider.index()]
            .iter()
            .enumerate()
        {
            seen.insert(path_key(path));
            listed.push(ListedSource {
                text: self.location_text(path),
                remove: Some((
                    format!("remove-home-{}-{index}", provider.key()),
                    UiAction::RemoveHome(provider, index),
                )),
            });
        }
        if provider == Provider::Cursor
            && let Some(path) = self.config.cursor_imports.first()
        {
            seen.insert(path_key(path));
            listed.push(ListedSource {
                text: self.location_text(path),
                remove: Some(("cursor-clear".into(), UiAction::ClearCursor)),
            });
        }
        for path in open_usage::providers::homes(provider, &self.config) {
            if !seen.insert(path_key(&path)) || !path.exists() {
                continue;
            }
            listed.push(ListedSource {
                text: self.location_text(&path),
                remove: None,
            });
        }
        listed
    }
    fn location_text(&self, path: &std::path::Path) -> String {
        if self.config.privacy {
            "Hidden".into()
        } else {
            display_path(path)
        }
    }
    pub(super) fn settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        self.page_body(cx)
            .child(panel(theme).child(title("Display",theme))
                .child(self.preference_row("Appearance","",self.button("theme-toggle",if self.config.dark{"Dark"}else{"Light"},Some("theme"),false,UiAction::Dark,cx)))
                .child(self.preference_row("Privacy mode","Hide amounts, tokens, projects and paths",self.button("privacy-toggle",if self.config.privacy{"Hidden"}else{"Visible"},Some("privacy"),self.config.privacy,UiAction::Privacy,cx))))
            .child(panel(theme).child(title("Data",theme))
                .child(div().mt(px(20.)).flex().items_center().justify_between().child(label("Automatic refresh",theme))
                    .child(div().flex().gap(px(6.)).children(open_usage::config::REFRESH_INTERVALS.into_iter().map(|seconds|self.button(format!("refresh-{seconds}"),if seconds==3600 { "1 hour".into() } else { format!("{} min",seconds/60) },None,self.config.refresh_seconds==seconds,UiAction::Interval(seconds),cx)))))
                .child(label("Cost uses reported amounts or API-rate estimates. Subscription fees are excluded.",theme).mt(px(20.)).text_size(px(12.))))
            .child(panel(theme).child(div().flex().justify_between().child(title(if cfg!(target_os="macos"){"Menu bar"}else{"System tray"},theme)).child(self.badge(if self.tray.is_some(){"Active"}else if self.tray_attempted{"Unavailable"}else{"Starting"},self.tray.is_some())))
                .child(label(if self.tray.is_some(){"Closing the window keeps tracking active."}else{"Closing the window quits the app."},theme).mt(px(12.)))
                .when_some(self.tray_error.as_ref(),|panel,error|panel.child(label(error.clone(),theme).mt(px(10.)).text_size(px(12.))))
                .child(div().flex().gap(px(8.)).mt(px(16.)).child(self.button("settings-quick","Quick view",Some("overview"),false,UiAction::Compact,cx)).child(self.button("settings-quit","Quit",None,false,UiAction::Quit,cx))))
    }
    fn preference_row(&self, name: &str, description: &str, control: impl IntoElement) -> Div {
        let theme = self.theme();
        div()
            .mt(px(16.))
            .pt(px(16.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(20.))
            .child(
                div()
                    .child(div().text_size(px(14.)).child(name.to_owned()))
                    .when(!description.is_empty(), |text| {
                        text.child(
                            label(description.to_owned(), theme)
                                .mt(px(5.))
                                .text_size(px(12.)),
                        )
                    }),
            )
            .child(control)
    }
    pub(super) fn compact_view(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let analysis =
            analytics::analyze(self.data(), &self.config, Period::Today, None, Utc::now());
        let accounts = self.visible_accounts();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.panel))
            .child(
                div()
                    .flex_shrink_0()
                    .px(px(18.))
                    .py(px(15.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .child(title("Open Usage", theme))
                    .child(div().flex_1())
                    .child(self.button(
                        "compact-full",
                        "Accounts",
                        Some("arrow"),
                        false,
                        UiAction::Page(Page::Overview),
                        cx,
                    )),
            )
            .child(
                div()
                    .id("quick-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(18.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(16.))
                            .child(label("Today", theme))
                            .child(label(
                                format!("{} tokens", self.number(analysis.totals.tokens.total())),
                                theme,
                            )),
                    )
                    .when(accounts.is_empty(), |body| {
                        body.child(label("No connected accounts", theme))
                    })
                    .children(accounts.iter().map(|status| {
                        let totals = &analysis.providers[status.provider.index()];
                        panel(theme)
                            .p(px(16.))
                            .mb(px(12.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(9.))
                                    .child(
                                        provider_mark(status.provider, self.config.dark)
                                            .size(px(29.)),
                                    )
                                    .child(
                                        title(status.provider.account_name(), theme)
                                            .text_size(px(14.)),
                                    ),
                            )
                            .child(
                                div()
                                    .mt(px(13.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(label(
                                        format!("{} tokens", self.number(totals.tokens.total())),
                                        theme,
                                    ))
                                    .child(
                                        div()
                                            .font_family(mono_font())
                                            .text_size(px(14.))
                                            .child(self.spend(totals)),
                                    ),
                            )
                            .child(
                                div().mt(px(15.)).flex().flex_col().gap(px(12.)).children(
                                    status.limits.iter().take(2).map(|limit| {
                                        self.quota(limit, status.stale, status.provider)
                                    }),
                                ),
                            )
                    })),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .p(px(14.))
                    .border_t_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        label(
                            self.data()
                                .refreshed_at
                                .map(|at| {
                                    format!("Updated {}", at.with_timezone(&Local).format("%H:%M"))
                                })
                                .unwrap_or_else(|| "Not synced".into()),
                            theme,
                        )
                        .text_size(px(11.)),
                    )
                    .child(div().flex_1())
                    .child(self.button(
                        "compact-refresh",
                        "Refresh",
                        Some("refresh"),
                        false,
                        UiAction::Refresh,
                        cx,
                    )),
            )
    }
}
struct ListedSource {
    text: String,
    remove: Option<(String, UiAction)>,
}
fn display_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    }
}
fn path_key(path: &std::path::Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}
fn reported_error(detail: &str) -> bool {
    const KNOWN: &[&str] = &[
        "Paused in Connections",
        "Account history · across your machines",
        "Local history + live account limits",
        "Live account limits",
        "Cached Cursor account history",
        "Detected on this device · no local usage yet",
        "Import a usage CSV or enable account checks",
        "Connect a source to see usage",
    ];
    if detail.is_empty() || KNOWN.contains(&detail) || detail.ends_with("history files · local") {
        return false;
    }
    !(detail.starts_with("Use ") && detail.contains("to create local history"))
}
fn plan_name(plan: &str) -> String {
    plan.split(['_', '-'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| format!("{}{}", first.to_uppercase(), chars.as_str()))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
/// How many activity rows fit under the header, toolbar, and pager.
fn activity_detail_height(width: f32, height: f32) -> f32 {
    let padding = layout::page_padding(width, height);
    let content_width = width - layout::sidebar_width(width) - padding * 2.;
    let filters = if content_width < 650. { 102. } else { 66. };
    (height - layout::HEADER_HEIGHT - padding * 2. - filters - 24. - 48. - 36. - 53. - 54. - 2.)
        .clamp(80., 300.)
}
fn rows_that_fit(width: f32, height: f32, detail_open: bool, _privacy: bool) -> usize {
    let padding = layout::page_padding(width, height);
    let content_width = width - layout::sidebar_width(width) - padding * 2.;
    let filters = if content_width < 650. { 102. } else { 66. };
    let mut chrome = layout::HEADER_HEIGHT + padding * 2. + filters + 12. + 48. + 36. + 53. + 2.;
    if detail_open {
        chrome += 12. + activity_detail_height(width, height);
    }
    ((height - chrome) / 54.).floor().max(1.) as usize
}
fn activity_pages(current: usize, total: usize) -> Vec<Option<usize>> {
    if total <= 7 {
        return (0..total).map(Some).collect();
    }
    let mut pages = vec![Some(0)];
    let from = current.saturating_sub(2).max(1);
    let to = (current + 2).min(total - 2);
    if from > 1 {
        pages.push(None);
    }
    for page in from..=to {
        pages.push(Some(page));
    }
    if to + 1 < total - 1 {
        pages.push(None);
    }
    pages.push(Some(total - 1));
    pages
}
fn grouped(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
fn reset_text(at: Option<chrono::DateTime<Utc>>) -> String {
    let Some(at) = at else {
        return "Reset not reported".into();
    };
    let remaining = at.signed_duration_since(Utc::now());
    if remaining.num_seconds() <= 0 {
        return "Window ended".into();
    }
    let minutes = remaining.num_minutes();
    if minutes < 60 {
        format!("Resets in {}m", minutes.max(1))
    } else if minutes < 1440 {
        format!("Resets in {}h {}m", minutes / 60, minutes % 60)
    } else {
        format!(
            "Resets {}",
            at.with_timezone(&Local).format("%b %-d, %H:%M")
        )
    }
}
