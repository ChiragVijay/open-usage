use super::*;
use chrono::Local;
use open_usage::{
    analytics::{Analysis, Totals},
    model::{LimitWindow, Provider, ProviderStatus, UsageRecord},
};

impl Dashboard {
    pub(super) fn sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(theme.sidebar))
            .border_r_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(52.))
                    .px(px(18.))
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
                    .child(title("Open Usage", theme).text_size(px(15.))),
            )
            .child(
                div()
                    .px(px(12.))
                    .pt(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .children(
                        [Page::Overview, Page::Models, Page::Limits, Page::Activity]
                            .into_iter()
                            .map(|page| self.navigation(page, cx)),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px(px(12.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(self.navigation(Page::Connections, cx))
                    .child(self.navigation(Page::Settings, cx)),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(theme.line))
                    .px(px(18.))
                    .py(px(16.))
                    .child(
                        label(concat!("v", env!("CARGO_PKG_VERSION")), theme).text_size(px(11.)),
                    ),
            )
    }
    fn navigation(&self, page: Page, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme();
        let active = self.page == page;
        div()
            .id(SharedString::from(format!("nav-{}", page.title())))
            .focusable()
            .tab_index(0)
            .h(px(30.))
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
            .child(page.title())
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
    pub(super) fn header(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let updated = self
            .data()
            .refreshed_at
            .map(|time| format!("Updated {}", time.with_timezone(&Local).format("%H:%M")))
            .unwrap_or_else(|| "Not synced".into());
        div()
            .h(px(52.))
            .flex_shrink_0()
            .px(px(24.))
            .bg(rgb(theme.bg))
            .border_b_1()
            .border_color(rgb(theme.line))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(title(self.page.title(), theme).text_size(px(16.)))
            .child(div().flex_1())
            .child(
                label(
                    if self.refreshing {
                        "Syncing…".into()
                    } else {
                        updated
                    },
                    theme,
                )
                .text_size(px(12.))
                .mr(px(4.)),
            )
            .child(self.button(
                "header-theme",
                if self.config.dark { "Dark" } else { "Light" },
                Some("theme"),
                false,
                UiAction::Dark,
                cx,
            ))
            .child(self.button(
                "header-refresh",
                "Refresh",
                Some("refresh"),
                false,
                UiAction::Refresh,
                cx,
            ))
            .child(self.button(
                "header-compact",
                "Quick view",
                Some("overview"),
                false,
                UiAction::Compact,
                cx,
            ))
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
    fn filters(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(self.button(
                "account-all",
                "All accounts",
                None,
                self.provider.is_none(),
                UiAction::Provider(None),
                cx,
            ))
            .children(self.visible_accounts().iter().map(|status| {
                self.button(
                    format!("account-{}", status.provider.key()),
                    status.provider.name(),
                    None,
                    self.provider == Some(status.provider),
                    UiAction::Provider(Some(status.provider)),
                    cx,
                )
            }))
    }
    fn toolbar(&self, cx: &Context<Self>, filter: bool) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .when(filter, |row| row.child(self.filters(cx)))
            .child(div().flex_1())
            .child(self.periods(cx))
    }
    fn metric(&self, name: &str, value: String, note: &str) -> Div {
        let theme = self.theme();
        div()
            .flex_1()
            .min_w_0()
            .child(label(name.to_owned(), theme).text_size(px(12.)))
            .child(
                div()
                    .mt(px(7.))
                    .font_family(display_font())
                    .text_size(px(25.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(value),
            )
            .child(label(note.to_owned(), theme).text_size(px(12.)).mt(px(5.)))
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
    pub(super) fn overview(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let now = Utc::now();
        let analysis = analytics::analyze(self.data(), &self.config, self.period, None, now);
        let accounts = self.visible_accounts();
        let body = self.page_body(cx).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(title("Account usage", theme))
                .child(
                    label(
                        format!(
                            "{} account{}",
                            accounts.len(),
                            if accounts.len() == 1 { "" } else { "s" }
                        ),
                        theme,
                    )
                    .text_size(px(12.)),
                )
                .child(div().flex_1())
                .child(self.periods(cx)),
        );
        if accounts.is_empty() {
            return body.child(
                panel(theme)
                    .py(px(36.))
                    .child(title("No agents detected", theme))
                    .child(
                        label(
                            "Open a supported agent, or add its local data folder.",
                            theme,
                        )
                        .mt(px(8.)),
                    )
                    .child(
                        self.button(
                            "empty-connect",
                            "Find agents",
                            Some("connections"),
                            false,
                            UiAction::Page(Page::Connections),
                            cx,
                        )
                        .mt(px(18.)),
                    ),
            );
        }
        let cost_note = self.cost_caption(&analysis.totals);
        let summary = div()
            .flex()
            .gap(px(32.))
            .py(px(6.))
            .child(self.metric(
                "Tokens",
                self.number(analysis.totals.tokens.total()),
                self.period.label(),
            ))
            .child(self.metric("Cost", self.spend(&analysis.totals), &cost_note))
            .child(self.metric(
                "Sessions",
                self.number(analysis.totals.sessions as u64),
                "Recorded session IDs",
            ));
        let table = panel(theme)
            .p_0()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .px(px(18.))
                    .py(px(11.))
                    .bg(rgb(theme.sidebar))
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .child(label("Account", theme).flex_1().min_w(px(180.)))
                    .child(label("Tokens", theme).w(px(100.)))
                    .child(label("Cost", theme).w(px(130.)))
                    .child(label("Allowance", theme).w(px(250.)))
                    .child(div().w(px(60.))),
            )
            .children(accounts.iter().map(|status| {
                self.account_row(status, &analysis.providers[status.provider.index()], cx)
            }));
        let total_tokens = analysis.totals.tokens.total().max(1) as f32;
        let share = div()
            .h(px(8.))
            .w_full()
            .rounded_full()
            .overflow_hidden()
            .bg(rgb(theme.line))
            .flex()
            .gap(px(2.))
            .when(!self.config.privacy, |bar| {
                bar.children(accounts.iter().map(|status| {
                    let tokens = analysis.providers[status.provider.index()].tokens.total();
                    div()
                        .h_full()
                        .w(relative(tokens as f32 / total_tokens))
                        .bg(rgb(color(status.provider, self.config.dark)))
                }))
            });
        body.child(summary)
            .child(share)
            .child(table)
            .child(self.chart(&analysis, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        label(
                            "USD · Reported usage only. Subscription fees excluded.",
                            theme,
                        )
                        .text_size(px(11.)),
                    )
                    .child(div().flex_1())
                    .child(self.button(
                        "account-export",
                        "Export",
                        Some("export"),
                        false,
                        UiAction::Export,
                        cx,
                    )),
            )
    }
    fn account_row(&self, status: &ProviderStatus, totals: &Totals, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let provider = status.provider;
        let reading = if !status.stale {
            "Live"
        } else if status.checked_at.is_some() {
            "Cached"
        } else if status.connected {
            "Local"
        } else {
            "Detected"
        };
        let has_history = status.connected
            || self
                .data()
                .records
                .iter()
                .any(|record| record.provider == provider);
        let description = status.plan.as_deref().map(plan_name).unwrap_or_else(|| {
            if has_history {
                if provider == Provider::Cursor {
                    "Account usage records".into()
                } else {
                    "Local agent history".into()
                }
            } else {
                "No usage recorded yet".into()
            }
        });
        div()
            .flex()
            .items_center()
            .gap(px(20.))
            .px(px(18.))
            .py(px(16.))
            .border_b_1()
            .border_color(rgb(theme.line))
            .child(
                div()
                    .flex_1()
                    .min_w(px(180.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(provider_mark(provider, self.config.dark).size(px(30.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(title(provider.account_name(), theme).text_size(px(13.)))
                            .child(label(description, theme).text_size(px(11.)).mt(px(4.)))
                            .child(
                                div()
                                    .flex()
                                    .mt(px(7.))
                                    .child(self.badge(reading, !status.stale)),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(100.))
                    .font_family(mono_font())
                    .text_size(px(14.))
                    .child(if has_history {
                        self.number(totals.tokens.total())
                    } else {
                        "—".into()
                    }),
            )
            .child(
                div()
                    .w(px(130.))
                    .child(
                        div()
                            .font_family(mono_font())
                            .text_size(px(14.))
                            .child(self.spend(totals)),
                    )
                    .child(
                        label(self.cost_caption(totals), theme)
                            .text_size(px(11.))
                            .mt(px(5.)),
                    ),
            )
            .child(
                div()
                    .w(px(250.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(13.))
                    .when(status.limits.is_empty(), |column| {
                        column.child(
                            label(
                                if status.detected && !status.connected {
                                    "Signed-in account not available"
                                } else {
                                    "No allowance reported"
                                },
                                theme,
                            )
                            .text_size(px(12.)),
                        )
                    })
                    .children(
                        status
                            .limits
                            .iter()
                            .take(2)
                            .map(|limit| self.quota(limit, status.stale, provider)),
                    ),
            )
            .child(div().w(px(60.)).child(self.button(
                format!("history-{}", provider.key()),
                "View",
                Some("arrow"),
                false,
                UiAction::AccountActivity(provider),
                cx,
            )))
    }
    fn quota(&self, limit: &LimitWindow, stale: bool, provider: Provider) -> Div {
        let theme = self.theme();
        let expired = limit.expired(Utc::now());
        let remaining = if self.config.privacy {
            "Hidden".into()
        } else if expired {
            "Refresh needed".into()
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
    fn chart(&self, analysis: &Analysis, cx: &Context<Self>) -> Div {
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
        let hover = self.hovered_day.and_then(|day| analysis.buckets.get(day));
        let caption = hover
            .map(|bucket| {
                format!(
                    "{} · {}",
                    format_args!(
                        "{}–{}",
                        self.period.bucket_label(bucket.start),
                        self.period.bucket_label(bucket.end)
                    ),
                    if self.token_chart {
                        format!("{} tokens", self.number(bucket.tokens.iter().sum()))
                    } else {
                        if bucket.cost_rows.iter().sum::<usize>() > 0 {
                            self.money(bucket.cost.iter().sum())
                        } else {
                            "No public rate".into()
                        }
                    }
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "{} · {}",
                    self.period.label(),
                    if self.token_chart {
                        format!("{} tokens", self.number(analysis.totals.tokens.total()))
                    } else {
                        self.spend(&analysis.totals)
                    }
                )
            });
        panel(theme)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(title(self.period.chart_title(), theme).text_size(px(13.)))
                    .child(div().flex_1())
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
                label(
                    if self.config.privacy {
                        "Values hidden".into()
                    } else {
                        caption
                    },
                    theme,
                )
                .text_size(px(12.))
                .mt(px(8.)),
            )
            .child(
                div()
                    .id("daily-plot")
                    .mt(px(20.))
                    .h(px(180.))
                    .border_b_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .items_end()
                    .gap(px(if analysis.buckets.len() > 30 { 2. } else { 5. }))
                    .on_hover(cx.listener(|view, hovered: &bool, _, cx| {
                        if !hovered && view.hovered_day.take().is_some() {
                            cx.notify();
                        }
                    }))
                    .when(self.config.privacy || maximum == 0.0, |plot| {
                        plot.items_center().justify_center().child(label(
                            if self.config.privacy {
                                "Values hidden"
                            } else if self.token_chart {
                                "No token usage in this period"
                            } else if analysis.totals.spend().is_some() {
                                "Cost is $0.00"
                            } else {
                                "No priced usage in this period"
                            },
                            theme,
                        ))
                    })
                    .when(!self.config.privacy && maximum > 0.0, |plot| {
                        plot.children(values.iter().enumerate().map(|(index, values)| {
                            let dimmed = self.hovered_day.is_some_and(|day| day != index);
                            div()
                                .id(SharedString::from(format!("day-{index}")))
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .justify_end()
                                .gap(px(1.))
                                .opacity(if dimmed { 0.45 } else { 1.0 })
                                .on_mouse_move(cx.listener(move |view, _, _, cx| {
                                    if view.hovered_day != Some(index) {
                                        view.hovered_day = Some(index);
                                        cx.notify();
                                    }
                                }))
                                .children(Provider::ALL.into_iter().rev().map(|provider| {
                                    div()
                                        .h(px((values[provider.index()] / maximum * 172.0) as f32))
                                        .w_full()
                                        .rounded(px(3.))
                                        .bg(rgb(color(provider, self.config.dark)))
                                }))
                        }))
                    }),
            )
            .child(
                div()
                    .mt(px(9.))
                    .flex()
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
                        .text_size(px(11.)),
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
                        .text_size(px(11.)),
                    ),
            )
            .child(div().mt(px(14.)).flex().gap(px(22.)).children(
                self.visible_accounts().iter().map(|status| {
                    let totals = &analysis.providers[status.provider.index()];
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(dot(status.provider, self.config.dark))
                        .child(label(status.provider.name(), theme).text_size(px(12.)))
                        .child(div().font_family(mono_font()).text_size(px(12.)).child(
                            if self.token_chart {
                                self.number(totals.tokens.total())
                            } else {
                                self.spend(totals)
                            },
                        ))
                }),
            ))
    }
    pub(super) fn models_view(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let analysis = analytics::analyze(
            self.data(),
            &self.config,
            self.period,
            self.provider,
            Utc::now(),
        );
        self.page_body(cx).child(self.toolbar(cx,true))
            .child(panel(theme).p(px(0.)).overflow_hidden().child(div().px(px(18.)).py(px(16.)).flex().justify_between().child(title("Model usage",theme)).child(label(format!("{} models",analysis.models.len()),theme)))
                .child(self.model_row("Model","Account","Tokens","Cache read","Cost",true))
                .when(analysis.models.is_empty(),|table|table.child(div().p(px(28.)).child(label("No model usage in this period",theme))))
                .children(analysis.models.iter().map(|row|self.model_row(&row.model,row.provider.name(),&self.number(row.totals.tokens.total()),&self.number(row.totals.tokens.cache_read),&self.spend(&row.totals),false))))
            .child(label("Models are grouped by the account that recorded the usage. No model pricing is applied.",theme).text_size(px(12.)))
    }
    fn model_row(
        &self,
        model: &str,
        account: &str,
        tokens: &str,
        cache: &str,
        cost: &str,
        heading: bool,
    ) -> Div {
        let theme = self.theme();
        div()
            .h(px(if heading { 38. } else { 48. }))
            .px(px(18.))
            .flex()
            .items_center()
            .gap(px(16.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .when(heading, |row| row.bg(rgb(theme.bg)))
            .child(
                self.table_cell(model, None, false, heading)
                    .flex_1()
                    .min_w_0(),
            )
            .child(self.table_cell(account, Some(105.), false, heading))
            .child(self.table_cell(tokens, Some(110.), true, heading))
            .child(self.table_cell(cache, Some(110.), true, heading))
            .child(self.table_cell(cost, Some(125.), true, heading))
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
    pub(super) fn limits_view(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let accounts = self.visible_accounts();
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
                            .children(status.limits.iter().map(|limit| {
                                div().flex_1().min_w_0().child(self.quota(
                                    limit,
                                    status.stale,
                                    status.provider,
                                ))
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
    pub(super) fn activity(&self, viewport_height: f32, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let records = self
            .data()
            .records
            .iter()
            .filter(|record| {
                self.config.enabled[record.provider.index()]
                    && analytics::matches(record, self.period, self.provider, Utc::now())
            })
            .collect::<Vec<_>>();
        let detail_open = self
            .detail
            .as_ref()
            .is_some_and(|id| records.iter().any(|record| record.id == *id));
        let fitted = rows_that_fit(viewport_height, detail_open, self.config.privacy);
        let page_size = self.activity_rows.unwrap_or(fitted);
        let scroll_rows = page_size > fitted;
        let page_count = records.len().div_ceil(page_size);
        let page = self.activity_page.min(page_count.saturating_sub(1));
        let start = page * page_size;
        self.page_body(cx)
            .flex_1()
            .min_h_0()
            .child(self.toolbar(cx, true).flex_shrink_0())
            .when_some(
                self.detail
                    .as_ref()
                    .and_then(|id| records.iter().find(|record| record.id == *id)),
                |body, record| body.child(self.usage_detail(record, cx).flex_shrink_0()),
            )
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
                            .flex_shrink_0()
                            .px(px(18.))
                            .py(px(16.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap(px(10.))
                                    .child(title("Usage events", theme))
                                    .child(
                                        label(
                                            format!(
                                                "{} event{}",
                                                grouped(records.len()),
                                                if records.len() == 1 { "" } else { "s" }
                                            ),
                                            theme,
                                        )
                                        .text_size(px(12.)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(self.activity_row_options(cx))
                                    .child(self.button(
                                        "activity-export",
                                        "Export CSV",
                                        Some("export"),
                                        false,
                                        UiAction::Export,
                                        cx,
                                    )),
                            ),
                    )
                    .child(
                        self.activity_columns(
                            ["Time", "Account", "Model", "Project", "Tokens", "Cost"],
                            true,
                            None,
                        )
                        .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "activity-rows-{page}-{page_size}"
                            )))
                            .flex_1()
                            .min_h_0()
                            .when(scroll_rows, |rows| rows.overflow_y_scroll())
                            .when(records.is_empty(), |rows| {
                                rows.child(
                                    div()
                                        .p(px(28.))
                                        .child(label("No usage events in this period", theme)),
                                )
                            })
                            .children(records.iter().skip(start).take(page_size).map(|record| {
                                let id = record.id.clone();
                                let cost = self.event_cost(record).0;
                                let selected = self.detail.as_deref() == Some(record.id.as_str());
                                div()
                                    .id(SharedString::from(format!("record-{id}")))
                                    .cursor_pointer()
                                    .bg(rgb(if selected { theme.wash } else { theme.panel }))
                                    .hover(move |style| style.bg(rgb(theme.wash)))
                                    .child(
                                        self.activity_columns(
                                            [
                                                &record
                                                    .timestamp
                                                    .with_timezone(&Local)
                                                    .format("%b %-d  %H:%M")
                                                    .to_string(),
                                                record.provider.name(),
                                                &record.model,
                                                if self.config.privacy {
                                                    "Hidden"
                                                } else {
                                                    &record.project
                                                },
                                                &self.number(record.tokens.total()),
                                                &cost,
                                            ],
                                            false,
                                            Some(record.provider),
                                        ),
                                    )
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.perform(UiAction::Detail(id.clone()), window, cx)
                                    }))
                            })),
                    )
                    .child(
                        self.activity_pager(page, page_count, start, page_size, records.len(), cx)
                            .flex_shrink_0(),
                    ),
            )
    }
    fn activity_row_options(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(label("Show", theme).text_size(px(12.)))
            .children(
                [
                    (None, "Fit"),
                    (Some(25), "25"),
                    (Some(50), "50"),
                    (Some(100), "100"),
                ]
                .into_iter()
                .map(|(limit, name)| {
                    self.button(
                        format!("activity-rows-{name}"),
                        name,
                        None,
                        self.activity_rows == limit,
                        UiAction::ActivityRows(limit),
                        cx,
                    )
                }),
            )
    }
    fn activity_pager(
        &self,
        page: usize,
        page_count: usize,
        start: usize,
        page_size: usize,
        total: usize,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme();
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
                    activity_pages(page, page_count)
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
    fn activity_columns(
        &self,
        columns: [&str; 6],
        heading: bool,
        provider: Option<open_usage::model::Provider>,
    ) -> Div {
        let [time, account, model, project, tokens, cost] = columns;
        let theme = self.theme();
        div()
            .h(px(if heading { 36. } else { 42. }))
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(12.))
            .border_t_1()
            .border_color(rgb(theme.line))
            .when(heading, |row| row.bg(rgb(theme.sidebar)))
            .child(self.table_cell(time, Some(118.), false, heading))
            .child(
                div()
                    .w(px(92.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .when(heading, |cell| {
                        cell.child(div().size(px(7.)).flex_shrink_0())
                    })
                    .when_some(provider, |cell, provider| {
                        cell.child(dot(provider, self.config.dark))
                    })
                    .child(
                        self.table_cell(account, None, false, heading)
                            .flex_1()
                            .min_w_0(),
                    ),
            )
            .child(
                self.table_cell(model, None, false, heading)
                    .flex_1()
                    .min_w_0(),
            )
            .child(self.table_cell(project, Some(118.), false, heading))
            .child(self.table_cell(tokens, Some(88.), true, heading))
            .child(self.table_cell(cost, Some(110.), true, heading))
    }
    fn usage_detail(&self, record: &UsageRecord, cx: &Context<Self>) -> Div {
        let theme = self.theme();
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
                        false,
                        UiAction::CloseDetail,
                        cx,
                    )),
            )
            .child(
                div()
                    .mt(px(16.))
                    .flex()
                    .gap(px(18.))
                    .child(self.metric("Account", record.provider.name().into(), &record.model))
                    .child({
                        let (amount, note) = self.event_cost(record);
                        self.metric("Cost", amount, note)
                    }),
            )
            .child(
                div()
                    .mt(px(20.))
                    .pt(px(16.))
                    .border_t_1()
                    .border_color(rgb(theme.line))
                    .flex()
                    .gap(px(16.))
                    .child(self.metric("Input", self.number(record.tokens.input), "Uncached"))
                    .child(self.metric(
                        "Output",
                        self.number(record.tokens.output),
                        "Includes reasoning",
                    ))
                    .child(self.metric(
                        "Cache read",
                        self.number(record.tokens.cache_read),
                        "Cached input",
                    ))
                    .child(self.metric(
                        "Cache write",
                        self.number(record.tokens.cache_write),
                        "Stored context",
                    )),
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
                .child(label("Account-reported cost is used when a provider sends one. Otherwise the cost is the public API rate for that model. Subscription fees are not included.",theme).mt(px(20.)).text_size(px(12.))))
            .child(panel(theme).child(div().flex().justify_between().child(title(if cfg!(target_os="macos"){"Menu bar"}else{"System tray"},theme)).child(self.badge(if self.tray.is_some(){"Active"}else if self.tray_attempted{"Unavailable"}else{"Starting"},self.tray.is_some())))
                .child(label(if self.tray.is_some(){"Click for quick view. Right-click for refresh and quit. Closing the window keeps usage tracking active."}else{"Closing the window quits the app when the system tray is unavailable."},theme).mt(px(12.)))
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
fn rows_that_fit(viewport_height: f32, detail_open: bool, privacy: bool) -> usize {
    const ROW: f32 = 43.;
    let mut chrome = 52. + 48. + 36. + 16. + 60. + 37. + 53. + 8.;
    if detail_open {
        chrome += 16. + if privacy { 290. } else { 360. };
    }
    ((viewport_height - chrome) / ROW).floor().max(1.) as usize
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
        return "Window ended · refresh needed".into();
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
