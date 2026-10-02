use super::*;
use chrono::{Duration as ChronoDuration, Local};
use open_usage::{
    analytics::{Analysis, Totals},
    model::{ProviderStatus, UsageRecord},
    pricing::price_for,
};

impl Dashboard {
    pub(super) fn sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .w(px(194.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(theme.sidebar))
            .border_r_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .p(px(18.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(53.))
                    .mb(px(26.))
                    .child(
                        div()
                            .size(px(33.))
                            .rounded(px(10.))
                            .bg(rgb(theme.accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path("brand").size(px(23.)).text_color(rgb(0xffffff))),
                    )
                    .child(
                        div()
                            .font_family(display_font())
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("open usage"),
                    ),
            )
            .child(
                label("WORKSPACE", theme)
                    .text_size(px(10.))
                    .mb(px(12.))
                    .px(px(10.)),
            )
            .children(
                [Page::Overview, Page::Activity, Page::Limits]
                    .into_iter()
                    .map(|page| {
                        let selected = self.page == page;
                        div()
                            .id(SharedString::from(format!("nav-{}", page.title())))
                            .focusable()
                            .tab_index(0)
                            .flex()
                            .gap(px(12.))
                            .items_center()
                            .h(px(43.))
                            .px(px(12.))
                            .mb(px(5.))
                            .rounded(px(9.))
                            .cursor_pointer()
                            .bg(rgb(if selected { theme.wash } else { theme.sidebar }))
                            .text_color(rgb(if selected { theme.accent } else { theme.muted }))
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .hover(move |s| s.bg(rgb(theme.wash)))
                            .focus(move |s| s.bg(rgb(theme.wash)))
                            .child(
                                svg().path(page.icon()).size(px(18.)).text_color(rgb(
                                    if selected { theme.accent } else { theme.muted },
                                )),
                            )
                            .child(page.title())
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.perform(UiAction::Page(page), window, cx)
                            }))
                            .on_key_down(cx.listener(
                                move |view, event: &KeyDownEvent, window, cx| {
                                    if event.keystroke.key == "enter" {
                                        view.perform(UiAction::Page(page), window, cx);
                                        cx.stop_propagation();
                                    }
                                },
                            ))
                    }),
            )
            .child(div().h(px(1.)).bg(rgb(theme.line)).my(px(22.)))
            .child(
                label("YOUR PROVIDERS", theme)
                    .text_size(px(10.))
                    .mb(px(13.))
                    .px(px(10.)),
            )
            .children(Provider::ALL.into_iter().map(|provider| {
                div()
                    .id(SharedString::from(format!("side-{}", provider.key())))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(34.))
                    .px(px(12.))
                    .cursor_pointer()
                    .child(dot(provider))
                    .child(div().text_color(rgb(theme.muted)).child(provider.name()))
                    .child(div().flex_1())
                    .child(div().size(px(5.)).rounded_full().bg(rgb(
                        if self.config.enabled[provider.index()] {
                            color(provider)
                        } else {
                            theme.line
                        },
                    )))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.provider = Some(provider);
                        view.perform(UiAction::Page(Page::Overview), window, cx);
                    }))
            }))
            .child(div().flex_1())
            .child(
                div()
                    .rounded(px(12.))
                    .bg(rgb(theme.wash))
                    .p(px(13.))
                    .mb(px(20.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .mb(px(6.))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(0x33958a)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(if self.demo {
                                        "Example workspace"
                                    } else {
                                        "Stored on this device"
                                    }),
                            ),
                    )
                    .child(
                        label(
                            if self.demo {
                                "Explore with sample data"
                            } else {
                                "Your history stays local"
                            },
                            theme,
                        )
                        .text_size(px(10.)),
                    ),
            )
            .children([Page::Connections, Page::Settings].into_iter().map(|page| {
                self.button(
                    format!("nav-bottom-{}", page.title()),
                    page.title(),
                    Some(page.icon()),
                    self.page == page,
                    UiAction::Page(page),
                    cx,
                )
                .justify_start()
                .mb(px(7.))
                .border_color(rgb(theme.sidebar))
                .bg(rgb(theme.sidebar))
            }))
            .child(
                label("v0.1.0  /  Native Rust", theme)
                    .text_size(px(10.))
                    .mt(px(16.))
                    .px(px(10.)),
            )
    }
    pub(super) fn header(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .h(px(72.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(theme.line))
            .px(px(30.))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .items_center()
                    .child(label("Workspace", theme))
                    .child(label("/", theme))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.page.title()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .child(
                        label(
                            if self.refreshing {
                                "Refreshing sources…"
                            } else if self.demo {
                                "Example data"
                            } else {
                                "Local workspace"
                            },
                            theme,
                        )
                        .mr(px(5.)),
                    )
                    .child(self.button(
                        "header-privacy",
                        if self.config.privacy {
                            "Hidden"
                        } else {
                            "Privacy"
                        },
                        Some("privacy"),
                        self.config.privacy,
                        UiAction::Privacy,
                        cx,
                    ))
                    .child(self.button(
                        "header-compact",
                        "Quick glance",
                        Some("overview"),
                        false,
                        UiAction::Compact,
                        cx,
                    )),
            )
    }
    fn periods(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .gap(px(3.))
            .p(px(3.))
            .rounded(px(10.))
            .bg(rgb(theme.line))
            .children(Period::ALL.into_iter().map(|period| {
                self.button(
                    format!("range-{}", period.days()),
                    period.label(),
                    None,
                    self.period == period,
                    UiAction::Period(period),
                    cx,
                )
                .border_color(rgb(if self.period == period {
                    theme.panel
                } else {
                    theme.line
                }))
                .bg(rgb(if self.period == period {
                    theme.panel
                } else {
                    theme.line
                }))
            }))
    }
    fn filters(&self, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(7.))
            .items_center()
            .child(self.button(
                "all-providers",
                "All providers",
                None,
                self.provider.is_none(),
                UiAction::Provider(None),
                cx,
            ))
            .children(Provider::ALL.into_iter().map(|provider| {
                self.button(
                    format!("filter-{}", provider.key()),
                    provider.name(),
                    None,
                    self.provider == Some(provider),
                    UiAction::Provider(Some(provider)),
                    cx,
                )
                .child(dot(provider))
            }))
            .child(div().flex_1())
            .child(self.button(
                "refresh",
                "Refresh",
                Some("refresh"),
                false,
                UiAction::Refresh,
                cx,
            ))
            .child(self.button(
                "export",
                "Export CSV",
                Some("export"),
                false,
                UiAction::Export,
                cx,
            ))
    }
    fn intro(
        &self,
        title_text: &str,
        subtitle: &str,
        cx: &Context<Self>,
        show_period: bool,
    ) -> Div {
        let theme = self.theme();
        div()
            .flex()
            .justify_between()
            .items_center()
            .mb(px(23.))
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(7.))
                    .child(
                        div()
                            .font_family(display_font())
                            .text_size(px(27.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title_text.to_owned()),
                    )
                    .child(label(subtitle.to_owned(), theme)),
            )
            .when(show_period, |div| div.child(self.periods(cx)))
    }
    fn demo_banner(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div().when(self.demo, |root| {
            root.flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(10.))
                .bg(rgb(theme.wash))
                .px(px(14.))
                .py(px(9.))
                .mb(px(18.))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme.accent))
                        .child("EXAMPLE DATA"),
                )
                .child(label(
                    "These numbers illustrate the app. Your live history is separate.",
                    theme,
                ))
                .child(div().flex_1())
                .child(self.button("leave-demo", "Use my data", None, false, UiAction::Demo, cx))
        })
    }
    fn stat(&self, label_text: &str, value: String, note: String, accent: Option<u32>) -> Div {
        let theme = self.theme();
        panel(theme)
            .flex_1()
            .min_w_0()
            .p(px(19.))
            .child(label(label_text.to_owned(), theme))
            .child(
                div()
                    .mt(px(10.))
                    .mb(px(5.))
                    .font_family(display_font())
                    .text_size(px(31.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(accent.unwrap_or(theme.ink)))
                    .child(value),
            )
            .child(label(note, theme).text_size(px(11.)))
    }
    pub(super) fn overview(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let analysis = analytics::analyze(
            self.data(),
            &self.config,
            self.period,
            self.provider,
            Utc::now(),
        );
        let total = &analysis.totals;
        let change = if analysis.previous_estimated > 0.0 {
            format!(
                "{:+.0}% vs previous period",
                (total.estimated / analysis.previous_estimated - 1.0) * 100.0
            )
        } else {
            "Based on available token history".into()
        };
        let cache = if total.tokens.total() > 0 {
            format!(
                "{:.0}% of tokens served from cache",
                total.tokens.cache_read as f64 / total.tokens.total() as f64 * 100.0
            )
        } else {
            "Cache reads are tracked separately".into()
        };
        div().flex().flex_col().gap(px(19.)).when(self.demo, |root| root.child(self.demo_banner(cx))).child(self.intro("Every token. In view.","A little clarity for your AI coding stack.",cx,true).mb(px(0.))).child(self.filters(cx))
            .child(div().flex().gap(px(14.)).child(self.stat("Estimated API-equivalent cost",self.money(total.estimated),if self.config.privacy {"Values hidden".into()}else{change},None)).child(self.stat("Tokens used",self.number(total.tokens.total()),format!("{} sessions · {} usage events",self.number(total.sessions as u64),self.number(total.events as u64)),None)).child(self.stat("Saved through cache",self.money(total.cache_savings),if self.config.privacy{"Values hidden".into()}else{cache},Some(0x33958a))))
            .child(div().flex().gap(px(16.)).child(self.chart(&analysis,cx).flex_1().min_w_0()).child(self.budget(cx)))
            .when(total.unpriced_rows>0,|root|root.child(div().rounded(px(10.)).bg(rgb(theme.wash)).p(px(12.)).flex().items_center().gap(px(10.)).child(label(format!("{} events have no known model price. Cost totals are partial; token totals include them.",total.unpriced_rows),theme)).child(div().flex_1()).child(self.button("unknown-prices","Model prices",None,false,UiAction::Page(Page::Settings),cx))))
            .child(div().flex().justify_between().items_center().child(title("Your providers",theme)).child(label("Click a provider to focus its history",theme)))
            .child(div().flex().gap(px(14.)).children(Provider::ALL.into_iter().filter(|p|self.provider.is_none_or(|selected|selected==*p)).map(|provider|self.provider_card(provider,&analysis.providers[provider.index()],cx))))
            .child(div().flex().gap(px(16.)).child(self.models(&analysis).flex_1().min_w_0()).child(self.token_mix(total).w(px(270.))))
            .child(label("API-equivalent estimates are not your subscription bill. Reported Cursor spend is shown separately. USD.",theme).text_size(px(11.)))
            .children(self.data().notices.iter().map(|notice|label(notice.clone(),theme)))
    }
    fn chart(&self, analysis: &Analysis, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let values: Vec<[f64; 3]> = analysis
            .buckets
            .iter()
            .map(|bucket| {
                if self.config.privacy {
                    [0.; 3]
                } else if self.token_chart {
                    bucket.tokens.map(|v| v as f64)
                } else {
                    bucket.cost
                }
            })
            .collect();
        let maximum = values
            .iter()
            .map(|values| values.iter().sum::<f64>())
            .fold(0.0, f64::max)
            .max(1.0);
        let caption = self
            .hovered_day
            .and_then(|i| analysis.buckets.get(i))
            .map(|bucket| {
                format!(
                    "{}  ·  {}",
                    bucket.date.format("%b %-d"),
                    if self.token_chart {
                        self.number(bucket.tokens.iter().sum())
                    } else {
                        self.money(bucket.cost.iter().sum())
                    }
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "{}  —  {}",
                    analysis.buckets.first().unwrap().date.format("%b %-d"),
                    analysis.buckets.last().unwrap().date.format("%b %-d")
                )
            });
        panel(theme)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(title("Usage rhythm", theme))
                            .child(label(caption, theme).text_size(px(11.))),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(3.))
                            .child(self.button(
                                "chart-cost",
                                "Cost",
                                None,
                                !self.token_chart,
                                UiAction::Metric(false),
                                cx,
                            ))
                            .child(self.button(
                                "chart-tokens",
                                "Tokens",
                                None,
                                self.token_chart,
                                UiAction::Metric(true),
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .h(px(190.))
                    .mt(px(18.))
                    .relative()
                    .flex()
                    .gap(px(14.))
                    .child(
                        div()
                            .w(px(36.))
                            .h_full()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .text_size(px(10.))
                            .text_color(rgb(theme.muted))
                            .child(if self.token_chart {
                                self.number(maximum as u64)
                            } else {
                                self.money(maximum)
                            })
                            .child(if self.token_chart {
                                self.number((maximum / 2.0) as u64)
                            } else {
                                self.money(maximum / 2.0)
                            })
                            .child(if self.config.privacy { "••" } else { "0" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .relative()
                            .child(
                                div()
                                    .absolute()
                                    .top(px(1.))
                                    .w_full()
                                    .h(px(1.))
                                    .bg(rgb(theme.line)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .top(px(94.))
                                    .w_full()
                                    .h(px(1.))
                                    .bg(rgb(theme.line)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(0.))
                                    .w_full()
                                    .h(px(1.))
                                    .bg(rgb(theme.line)),
                            )
                            .child(
                                div()
                                    .relative()
                                    .size_full()
                                    .flex()
                                    .items_end()
                                    .gap(px(if values.len() > 40 { 2. } else { 5. }))
                                    .children(values.iter().enumerate().map(|(index, values)| {
                                        div()
                                            .id(("chart-day", index))
                                            .flex_1()
                                            .min_w_0()
                                            .h_full()
                                            .flex()
                                            .flex_col()
                                            .justify_end()
                                            .gap(px(1.))
                                            .cursor_pointer()
                                            .opacity(
                                                if self.hovered_day.is_none_or(|day| day == index) {
                                                    1.0
                                                } else {
                                                    0.45
                                                },
                                            )
                                            .children(Provider::ALL.into_iter().rev().map(
                                                |provider| {
                                                    div()
                                                        .w_full()
                                                        .h(px((values[provider.index()] / maximum
                                                            * 176.)
                                                            as f32))
                                                        .min_h(px(
                                                            if values[provider.index()] > 0.0 {
                                                                2.
                                                            } else {
                                                                0.
                                                            },
                                                        ))
                                                        .rounded(px(2.))
                                                        .bg(rgb(color(provider)))
                                                },
                                            ))
                                            .on_hover(cx.listener(
                                                move |view, hovered: &bool, _, cx| {
                                                    view.hovered_day =
                                                        if *hovered { Some(index) } else { None };
                                                    cx.notify();
                                                },
                                            ))
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(15.))
                    .mt(px(15.))
                    .ml(px(50.))
                    .children(Provider::ALL.into_iter().map(|provider| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(dot(provider))
                            .child(label(provider.name(), theme).text_size(px(10.)))
                    })),
            )
    }
    fn budget(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let totals =
            analytics::analyze(self.data(), &self.config, Period::Month, None, Utc::now()).totals;
        let percent = (totals.estimated / self.config.monthly_budget * 100.0).min(100.0);
        panel(theme)
            .w(px(224.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(title("Keep your pace", theme))
                    .child(icon("limits", theme)),
            )
            .child(
                label("30-day estimate budget", theme)
                    .mt(px(7.))
                    .text_size(px(11.)),
            )
            .child(
                div()
                    .font_family(display_font())
                    .text_size(px(29.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .mt(px(24.))
                    .child(self.money((self.config.monthly_budget - totals.estimated).max(0.))),
            )
            .child(
                label("of your estimate budget left", theme)
                    .text_size(px(11.))
                    .mt(px(5.)),
            )
            .child(
                div()
                    .h(px(7.))
                    .rounded_full()
                    .bg(rgb(theme.line))
                    .mt(px(18.))
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(relative(if self.config.privacy {
                                0.0
                            } else {
                                percent as f32 / 100.0
                            }))
                            .bg(rgb(theme.accent)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mt(px(10.))
                    .child(label(self.money(totals.estimated), theme).text_size(px(11.)))
                    .child(label(self.money(self.config.monthly_budget), theme).text_size(px(11.))),
            )
            .child(div().flex_1().min_h(px(14.)))
            .child(
                label(
                    if totals.unpriced_rows > 0 {
                        "Some model prices are missing"
                    } else if percent >= 100.0 {
                        "Your estimate budget is reached"
                    } else {
                        "A guide to usage, not a bill"
                    },
                    theme,
                )
                .text_size(px(11.)),
            )
            .child(
                self.button(
                    "budget-settings",
                    "Adjust budget",
                    None,
                    false,
                    UiAction::Page(Page::Settings),
                    cx,
                )
                .mt(px(14.))
                .w_full(),
            )
    }
    fn provider_card(
        &self,
        provider: Provider,
        totals: &Totals,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        let status = &self.data().providers[provider.index()];
        panel(theme)
            .id(SharedString::from(format!(
                "provider-card-{}",
                provider.key()
            )))
            .flex_1()
            .min_w_0()
            .cursor_pointer()
            .hover(move |style| style.border_color(rgb(color(provider))))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.provider = if view.provider == Some(provider) {
                    None
                } else {
                    Some(provider)
                };
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .items_center()
                    .child(provider_mark(provider, self.config.dark))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(title(provider.name(), theme))
                            .child(
                                label(
                                    status.plan.clone().unwrap_or_else(|| {
                                        if status.connected {
                                            "Local history"
                                        } else {
                                            "Not connected"
                                        }
                                        .into()
                                    }),
                                    theme,
                                )
                                .text_size(px(10.)),
                            ),
                    )
                    .child(div().flex_1())
                    .child(icon("arrow", theme)),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .mt(px(18.))
                    .items_end()
                    .child(
                        div()
                            .font_family(display_font())
                            .text_size(px(25.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.money(totals.estimated)),
                    )
                    .child(
                        label(
                            format!("{} tokens", self.number(totals.tokens.total())),
                            theme,
                        )
                        .text_size(px(11.)),
                    ),
            )
            .child(
                label("Estimated API equivalent", theme)
                    .text_size(px(10.))
                    .mt(px(3.)),
            )
            .when(totals.reported_rows > 0, |card| {
                card.child(
                    label(
                        format!("{} reported by Cursor", self.money(totals.reported)),
                        theme,
                    )
                    .text_size(px(10.))
                    .mt(px(8.)),
                )
            })
            .child(div().h(px(1.)).bg(rgb(theme.line)).my(px(13.)))
            .child(if let Some(limit) = status.limits.first() {
                let expired = limit.expired(Utc::now());
                div()
                    .flex()
                    .justify_between()
                    .child(
                        label(
                            format!(
                                "{}{}",
                                limit.label,
                                if status.stale || expired {
                                    " · cached"
                                } else {
                                    ""
                                }
                            ),
                            theme,
                        )
                        .text_size(px(10.)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(color(provider)))
                            .child(if self.config.privacy {
                                "••••".into()
                            } else if expired {
                                "Refresh needed".into()
                            } else {
                                format!("{:.0}% left", limit.remaining())
                            }),
                    )
            } else {
                label(
                    if self.refreshing {
                        "Checking sources…"
                    } else if !self.config.online {
                        "Account checks are off"
                    } else {
                        "Limits unavailable"
                    },
                    theme,
                )
                .text_size(px(11.))
            })
    }
    fn models(&self, analysis: &Analysis) -> Div {
        let theme = self.theme();
        panel(theme)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb(px(17.))
                    .child(title("Model breakdown", theme))
                    .child(label("Estimated cost", theme).text_size(px(11.))),
            )
            .when(analysis.models.is_empty(), |panel| {
                panel.child(
                    label(
                        "Your models will appear after your first usage scan.",
                        theme,
                    )
                    .py(px(25.)),
                )
            })
            .children(analysis.models.iter().take(7).map(|row| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .py(px(10.))
                    .border_t_1()
                    .border_color(rgb(theme.line))
                    .child(dot(row.provider))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .child(row.model.clone()),
                    )
                    .child(
                        label(self.number(row.totals.tokens.total()), theme)
                            .font_family(mono_font())
                            .text_size(px(10.))
                            .w(px(70.))
                            .text_right(),
                    )
                    .child(
                        div()
                            .w(px(78.))
                            .text_right()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(if row.totals.unpriced_rows > 0 {
                                "Unpriced".into()
                            } else {
                                self.money(row.totals.estimated)
                            }),
                    )
            }))
    }
    fn token_mix(&self, totals: &Totals) -> Div {
        let theme = self.theme();
        let total = totals.tokens.total().max(1) as f32;
        panel(theme)
            .child(title("Inside your tokens", theme))
            .child(
                label("Cache is included in the total", theme)
                    .text_size(px(10.))
                    .mt(px(7.)),
            )
            .children(
                [
                    ("Input", totals.tokens.input, 0x7ba7da),
                    ("Output", totals.tokens.output, 0x8975d5),
                    ("Cache read", totals.tokens.cache_read, 0x33958a),
                    ("Cache write", totals.tokens.cache_write, 0xd18d68),
                ]
                .into_iter()
                .map(|(name, value, color)| {
                    div()
                        .mt(px(17.))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .mb(px(7.))
                                .child(label(name, theme).text_size(px(11.)))
                                .child(div().text_size(px(11.)).child(self.number(value))),
                        )
                        .child(
                            div().h(px(4.)).rounded_full().bg(rgb(theme.line)).child(
                                div()
                                    .h_full()
                                    .w(relative(if self.config.privacy {
                                        0.0
                                    } else {
                                        value as f32 / total
                                    }))
                                    .rounded_full()
                                    .bg(rgb(color)),
                            ),
                        )
                }),
            )
    }
    pub(super) fn limits_view(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div().flex().flex_col().gap(px(18.)).when(self.demo, |root| root.child(self.demo_banner(cx))).child(self.intro("Room to keep building.","Your subscription allowances, with the next reset in sight.",cx,false).mb(px(0.)))
            .child(div().flex().items_center().gap(px(12.)).child(label(if self.config.online{"Account limits refresh with your sources"}else{"Enable account checks to see live allowances"},theme)).child(div().flex_1()).child(self.button("limits-online",if self.config.online{"Account checks on"}else{"Enable account checks"},Some("connections"),self.config.online,UiAction::Online,cx)).child(self.button("limits-refresh","Refresh",Some("refresh"),false,UiAction::Refresh,cx)))
            .children(self.data().providers.iter().filter(|status|self.config.enabled[status.provider.index()]).map(|status|self.limit_panel(status,cx)))
            .child(label("Expired windows need a fresh account reading. Cached percentages do not predict your next allowance.",theme).text_size(px(11.)))
    }
    fn limit_panel(&self, status: &ProviderStatus, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        panel(theme)
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .items_center()
                    .child(provider_mark(status.provider, self.config.dark))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(title(status.provider.name(), theme))
                            .child(label(
                                status
                                    .plan
                                    .clone()
                                    .unwrap_or_else(|| "Subscription limits".into()),
                                theme,
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        label(
                            if status.stale {
                                "Cached / unavailable"
                            } else {
                                "Current account reading"
                            },
                            theme,
                        )
                        .text_size(px(11.)),
                    )
                    .child(self.button(
                        format!("dashboard-{}", status.provider.key()),
                        "Open account",
                        Some("arrow"),
                        false,
                        UiAction::OpenProvider(status.provider),
                        cx,
                    )),
            )
            .when(status.limits.is_empty(), |panel| {
                panel.child(
                    div()
                        .bg(rgb(theme.bg))
                        .rounded(px(11.))
                        .mt(px(20.))
                        .p(px(20.))
                        .child(label(status.detail.clone(), theme)),
                )
            })
            .child(div().flex().flex_wrap().gap(px(18.)).mt(px(22.)).children(
                status.limits.iter().map(|limit| {
                    let expired = limit.expired(Utc::now());
                    let reset = reset_text(limit.resets_at);
                    div()
                        .flex_1()
                        .min_w(px(215.))
                        .max_w(px(330.))
                        .rounded(px(12.))
                        .bg(rgb(theme.bg))
                        .p(px(19.))
                        .child(label(limit.label.clone(), theme))
                        .child(
                            div()
                                .font_family(display_font())
                                .text_size(px(30.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .mt(px(12.))
                                .mb(px(4.))
                                .child(if self.config.privacy {
                                    "••••".into()
                                } else if expired {
                                    "—".into()
                                } else {
                                    format!("{:.0}%", limit.remaining())
                                }),
                        )
                        .child(
                            label(
                                if expired {
                                    "Window ended · refresh needed"
                                } else {
                                    "of allowance left"
                                },
                                theme,
                            )
                            .text_size(px(11.)),
                        )
                        .child(
                            div()
                                .h(px(6.))
                                .rounded_full()
                                .bg(rgb(theme.line))
                                .mt(px(19.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(if self.config.privacy || expired {
                                            0.0
                                        } else {
                                            limit.remaining() as f32 / 100.0
                                        }))
                                        .bg(rgb(if limit.remaining() < 15.0 {
                                            0xc96666
                                        } else {
                                            color(status.provider)
                                        })),
                                ),
                        )
                        .child(label(reset, theme).text_size(px(11.)).mt(px(12.)))
                }),
            ))
            .child(
                label(
                    status
                        .checked_at
                        .map(|at| {
                            format!(
                                "Checked {}{}",
                                at.with_timezone(&Local).format("%b %-d, %-I:%M %p"),
                                if status.stale {
                                    " · cached values retained"
                                } else {
                                    ""
                                }
                            )
                        })
                        .unwrap_or_else(|| "No account reading yet".into()),
                    theme,
                )
                .text_size(px(10.))
                .mt(px(18.)),
            )
    }
    pub(super) fn activity(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        let records: Vec<&UsageRecord> = self
            .data()
            .records
            .iter()
            .filter(|record| {
                self.config.enabled[record.provider.index()]
                    && analytics::matches(record, self.period, self.provider, Utc::now())
            })
            .collect();
        let offset = self.activity_page * 40;
        let selected = self
            .detail
            .as_ref()
            .and_then(|id| records.iter().find(|r| &r.id == id));
        div()
            .flex()
            .flex_col()
            .gap(px(19.))
            .when(self.demo, |root| root.child(self.demo_banner(cx)))
            .child(
                self.intro(
                    "The work behind the numbers.",
                    "A timeline of usage events, with the source behind each reading.",
                    cx,
                    true,
                )
                .mb(px(0.)),
            )
            .child(self.filters(cx))
            .when_some(selected, |root, record| {
                root.child(
                    panel(theme)
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(title("Usage detail", theme))
                                .child(self.button(
                                    "close-detail",
                                    "Close",
                                    None,
                                    false,
                                    UiAction::CloseDetail,
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(30.))
                                .mt(px(16.))
                                .child(self.token_mix(&record_totals(record, &self.config)))
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap(px(12.))
                                        .child(title(record.model.clone(), theme))
                                        .child(label(
                                            if self.config.privacy {
                                                "Project hidden".into()
                                            } else {
                                                format!("Project: {}", record.project)
                                            },
                                            theme,
                                        ))
                                        .child(label(
                                            if self.config.privacy {
                                                "Source hidden".into()
                                            } else {
                                                format!("Source: {}", record.source)
                                            },
                                            theme,
                                        ))
                                        .child(label(
                                            format!(
                                                "Estimated API equivalent: {}",
                                                price_for(&record.model, &self.config.prices)
                                                    .map(|price| self
                                                        .money(price.cost(&record.tokens)))
                                                    .unwrap_or_else(|| "Unpriced".into())
                                            ),
                                            theme,
                                        ))
                                        .child(label(
                                            format!(
                                                "Reported spend: {}",
                                                record
                                                    .reported_cost
                                                    .map(|cost| self.money(cost))
                                                    .unwrap_or_else(|| "Not reported".into())
                                            ),
                                            theme,
                                        )),
                                ),
                        ),
                )
            })
            .child(
                panel(theme)
                    .p(px(0.))
                    .overflow_hidden()
                    .child(
                        div()
                            .px(px(22.))
                            .py(px(17.))
                            .flex()
                            .justify_between()
                            .child(title("Recent activity", theme))
                            .child(label(
                                format!(
                                    "{} usage events · newest first",
                                    self.number(records.len() as u64)
                                ),
                                theme,
                            )),
                    )
                    .child(activity_row(
                        "Time",
                        "Provider / model",
                        "Project",
                        "Tokens",
                        "Estimated",
                        theme,
                        true,
                    ))
                    .when(records.is_empty(), |panel| {
                        panel.child(
                            div()
                                .py(px(70.))
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(12.))
                                .child(title("Your next session starts here", theme))
                                .child(label(
                                    "Connect a source, or choose a wider time range.",
                                    theme,
                                ))
                                .child(self.button(
                                    "activity-connect",
                                    "Manage connections",
                                    Some("connections"),
                                    false,
                                    UiAction::Page(Page::Connections),
                                    cx,
                                )),
                        )
                    })
                    .children(records.iter().skip(offset).take(40).map(|record| {
                        div()
                            .id(SharedString::from(format!("record-{}", record.id)))
                            .cursor_pointer()
                            .hover(move |style| style.bg(rgb(theme.bg)))
                            .on_click(cx.listener({
                                let id = record.id.clone();
                                move |view, _, window, cx| {
                                    view.perform(UiAction::Detail(id.clone()), window, cx)
                                }
                            }))
                            .child(activity_row(
                                &record
                                    .timestamp
                                    .with_timezone(&Local)
                                    .format("%b %-d  %H:%M")
                                    .to_string(),
                                &format!("{}  /  {}", record.provider.name(), record.model),
                                if self.config.privacy {
                                    "••••"
                                } else if record.project.is_empty() {
                                    "—"
                                } else {
                                    &record.project
                                },
                                &self.number(record.tokens.total()),
                                &price_for(&record.model, &self.config.prices)
                                    .map(|price| self.money(price.cost(&record.tokens)))
                                    .unwrap_or_else(|| "Unpriced".into()),
                                theme,
                                false,
                            ))
                    }))
                    .child(
                        div()
                            .px(px(22.))
                            .py(px(15.))
                            .border_t_1()
                            .border_color(rgb(theme.line))
                            .flex()
                            .items_center()
                            .gap(px(9.))
                            .child(label(
                                format!(
                                    "Showing {}–{} of {}",
                                    if records.is_empty() { 0 } else { offset + 1 },
                                    (offset + 40).min(records.len()),
                                    records.len()
                                ),
                                theme,
                            ))
                            .child(div().flex_1())
                            .when(self.activity_page > 0, |row| {
                                row.child(self.button(
                                    "activity-prev",
                                    "Previous",
                                    None,
                                    false,
                                    UiAction::PreviousActivity,
                                    cx,
                                ))
                            })
                            .when(offset + 40 < records.len(), |row| {
                                row.child(self.button(
                                    "activity-next",
                                    "Next",
                                    None,
                                    false,
                                    UiAction::NextActivity,
                                    cx,
                                ))
                            }),
                    ),
            )
    }
    pub(super) fn connections(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div().flex().flex_col().gap(px(18.)).child(self.intro("Bring your stack together.","Use your existing history and signed-in accounts. No new API keys needed.",cx,false).mb(px(0.)))
            .child(panel(theme).flex().items_center().gap(px(18.)).child(div().flex_1().child(title("Account checks",theme)).child(label("Reads existing logins to check provider allowances and Cursor account history.",theme).mt(px(7.))).child(label("macOS may ask for Keychain access. Credentials are never saved by Open Usage.",theme).text_size(px(11.)).mt(px(6.)))).child(self.button("connections-online",if self.config.online{"Enabled"}else{"Enable"},None,self.config.online,UiAction::Online,cx)))
            .children(Provider::ALL.into_iter().map(|provider|{
                let status=&self.live.providers[provider.index()];
                panel(theme).child(div().flex().gap(px(13.)).items_center().child(provider_mark(provider,self.config.dark)).child(div().flex_1().child(title(provider.name(),theme)).child(label(match provider{Provider::Codex=>"Codex CLI & desktop · ChatGPT subscription limits",Provider::Claude=>"Claude Code history · Claude subscription limits",Provider::Cursor=>"Cursor account history across machines · monthly allowance"},theme).mt(px(5.)).text_size(px(11.)))).child(self.button(format!("enabled-{}",provider.key()),if self.config.enabled[provider.index()]{"Tracking on"}else{"Paused"},None,self.config.enabled[provider.index()],UiAction::Enable(provider),cx)))
                    .child(div().rounded(px(10.)).bg(rgb(theme.bg)).p(px(14.)).my(px(17.)).child(label(status.detail.clone(),theme)))
                    .child(div().flex().gap(px(9.)).child(self.button(format!("add-folder-{}",provider.key()),"Add data folder",Some("connections"),false,UiAction::AddHome(provider),cx)).when(provider==Provider::Cursor,|row|row.child(self.button("cursor-import","Import usage CSV",Some("export"),false,UiAction::ImportCursor,cx))).when(provider==Provider::Cursor&&!self.config.cursor_imports.is_empty(),|row|row.child(self.button("cursor-clear","Remove import",None,false,UiAction::ClearCursor,cx))).child(div().flex_1()).child(self.button(format!("connect-open-{}",provider.key()),"Open account",Some("arrow"),false,UiAction::OpenProvider(provider),cx)))
                    .when(!self.config.custom_homes[provider.index()].is_empty(),|card|card.child(label(format!("{} additional data folders",self.config.custom_homes[provider.index()].len()),theme).mt(px(12.)).text_size(px(11.))))
            }))
            .child(label("Cursor CSV imports take precedence over account history so the same events are not counted twice. Importing replaces the previous CSV.",theme).text_size(px(11.)))
            .child(panel(theme).flex().items_center().justify_between().child(div().child(title("Take a look around",theme)).child(label("Explore every screen using clearly marked example data.",theme).mt(px(7.)))).child(self.button("connections-demo",if self.demo{"Use my data"}else{"Explore example data"},None,self.demo,UiAction::Demo,cx)))
    }
    pub(super) fn settings(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme();
        div().flex().flex_col().gap(px(18.)).child(self.intro("Make it yours.","A few useful preferences. Everything is saved on this device.",cx,false).mb(px(0.)))
            .child(panel(theme).child(title("Appearance & privacy",theme)).child(div().flex().items_center().justify_between().mt(px(22.)).child(div().child(title("Color theme",theme)).child(label("Choose a light or dark workspace",theme).mt(px(5.)))).child(self.button("settings-dark",if self.config.dark{"Dark"}else{"Light"},Some("theme"),self.config.dark,UiAction::Dark,cx)))
                .child(div().flex().items_center().justify_between().border_t_1().border_color(rgb(theme.line)).pt(px(19.)).mt(px(19.)).child(div().child(title("Privacy mode",theme)).child(label("Hide amounts, charts, project names, and source paths · Ctrl/Cmd+P",theme).mt(px(5.)))).child(self.button("settings-privacy",if self.config.privacy{"Hidden"}else{"Visible"},Some("privacy"),self.config.privacy,UiAction::Privacy,cx))))
            .child(panel(theme).child(title("Usage preferences",theme)).child(label("Estimate budget · USD for the past 30 days",theme).mt(px(19.))).child(div().flex().gap(px(7.)).mt(px(10.)).children([50.,100.,250.,500.,1000.].into_iter().map(|budget|self.button(format!("budget-{budget}"),format!("${budget:.0}"),None,self.config.monthly_budget==budget,UiAction::Budget(budget),cx))))
                .child(label("Automatic refresh",theme).mt(px(24.))).child(div().flex().gap(px(7.)).mt(px(10.)).children([(60,"1 minute"),(300,"5 minutes"),(900,"15 minutes")].into_iter().map(|(seconds,name)|self.button(format!("interval-{seconds}"),name,None,self.config.refresh_seconds==seconds,UiAction::Interval(seconds),cx)))))
            .child(panel(theme).child(title("Model prices",theme)).child(label("Import exact model IDs and USD rates per million tokens. Overrides apply to all history.",theme).mt(px(9.))).child(label("Unknown models stay unpriced. The baseline uses standard rates; long-context and fast-mode premiums may differ.",theme).text_size(px(11.)).mt(px(6.)))
                .child(div().flex().gap(px(8.)).mt(px(20.)).child(self.button("price-template","Get CSV template",Some("export"),false,UiAction::PriceTemplate,cx)).child(self.button("price-import","Import model prices",Some("connections"),false,UiAction::ImportPrices,cx)).when(!self.config.prices.is_empty(),|row|row.child(self.button("price-reset","Reset to baseline",None,false,UiAction::ResetPrices,cx))))
                .children(self.config.prices.iter().map(|(model,price)|label(format!("{model}  ·  input ${:.2} / output ${:.2}",price.input,price.output),theme).mt(px(12.)))))
            .child(panel(theme).child(title("Keyboard shortcuts",theme)).child(div().flex().gap(px(40.)).mt(px(17.)).child(label("Ctrl/Cmd + 1–5    Switch view\nCtrl/Cmd + R        Refresh usage\nCtrl/Cmd + P        Privacy mode",theme).line_height(px(25.)).whitespace_normal()).child(label("C / T                    Cost / tokens\nL                           Subscription limits\nTab / Shift+Tab     Move between controls",theme).line_height(px(25.)).whitespace_normal())))
            .child(label("Open Usage 0.1.0 · Rust + GPUI · No telemetry or cloud sync",theme).text_size(px(11.)))
    }
    pub(super) fn compact_view(&self, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme();
        let analysis =
            analytics::analyze(self.data(), &self.config, Period::Today, None, Utc::now());
        div()
            .size_full()
            .id("compact-scroll")
            .overflow_y_scroll()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(title("open usage", theme))
                    .child(self.button(
                        "compact-full",
                        "Dashboard",
                        Some("arrow"),
                        false,
                        UiAction::Page(Page::Overview),
                        cx,
                    )),
            )
            .when(self.demo, |root| {
                root.child(label("EXAMPLE DATA", theme).text_color(rgb(theme.accent)))
            })
            .child(
                panel(theme)
                    .child(label("Today · estimated API equivalent", theme))
                    .child(
                        div()
                            .text_size(px(37.))
                            .font_family(display_font())
                            .font_weight(FontWeight::SEMIBOLD)
                            .mt(px(11.))
                            .child(self.money(analysis.totals.estimated)),
                    )
                    .child(
                        label(
                            format!(
                                "{} tokens · {} sessions",
                                self.number(analysis.totals.tokens.total()),
                                self.number(analysis.totals.sessions as u64)
                            ),
                            theme,
                        )
                        .mt(px(7.)),
                    ),
            )
            .children(
                self.data()
                    .providers
                    .iter()
                    .filter(|status| self.config.enabled[status.provider.index()])
                    .map(|status| {
                        panel(theme)
                            .p(px(15.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(provider_mark(status.provider, self.config.dark))
                                    .child(title(status.provider.name(), theme))
                                    .child(div().flex_1())
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(
                                        self.money(
                                            analysis.providers[status.provider.index()].estimated,
                                        ),
                                    )),
                            )
                            .when(status.limits.is_empty(), |card| {
                                card.child(
                                    label("Account limits unavailable", theme)
                                        .mt(px(11.))
                                        .text_size(px(11.)),
                                )
                            })
                            .children(status.limits.iter().take(2).map(|limit| {
                                div()
                                    .mt(px(12.))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .child(
                                                label(limit.label.clone(), theme)
                                                    .text_size(px(11.)),
                                            )
                                            .child(
                                                label(
                                                    if self.config.privacy {
                                                        "••••".into()
                                                    } else if limit.expired(Utc::now()) {
                                                        "Refresh needed".into()
                                                    } else {
                                                        format!(
                                                            "{:.0}% left{}",
                                                            limit.remaining(),
                                                            if status.stale {
                                                                " · cached"
                                                            } else {
                                                                ""
                                                            }
                                                        )
                                                    },
                                                    theme,
                                                )
                                                .text_size(px(11.)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h(px(4.))
                                            .bg(rgb(theme.line))
                                            .rounded_full()
                                            .mt(px(6.))
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(
                                                        if self.config.privacy
                                                            || limit.expired(Utc::now())
                                                        {
                                                            0.0
                                                        } else {
                                                            limit.remaining() as f32 / 100.
                                                        },
                                                    ))
                                                    .rounded_full()
                                                    .bg(rgb(color(status.provider))),
                                            ),
                                    )
                            }))
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(7.))
                    .child(
                        self.button(
                            "compact-refresh",
                            "Refresh",
                            Some("refresh"),
                            false,
                            UiAction::Refresh,
                            cx,
                        )
                        .flex_1(),
                    )
                    .child(self.button(
                        "compact-privacy",
                        "Privacy",
                        Some("privacy"),
                        self.config.privacy,
                        UiAction::Privacy,
                        cx,
                    ))
                    .child(self.button(
                        "compact-theme",
                        "Theme",
                        Some("theme"),
                        self.config.dark,
                        UiAction::Dark,
                        cx,
                    )),
            )
            .child(
                label(
                    if self.refreshing {
                        "Refreshing…"
                    } else {
                        "Click the tray icon to return here"
                    },
                    theme,
                )
                .text_size(px(10.)),
            )
    }
}

fn reset_text(at: Option<chrono::DateTime<Utc>>) -> String {
    let Some(at) = at else {
        return "Reset time not reported".into();
    };
    let delta = at - Utc::now();
    if delta <= ChronoDuration::zero() {
        "Window ended · refresh to check".into()
    } else if delta.num_days() > 0 {
        format!(
            "Resets in {}d {}h",
            delta.num_days(),
            delta.num_hours() % 24
        )
    } else if delta.num_hours() > 0 {
        format!(
            "Resets in {}h {}m",
            delta.num_hours(),
            delta.num_minutes() % 60
        )
    } else {
        format!("Resets in {}m", delta.num_minutes().max(1))
    }
}
fn activity_row(
    time: &str,
    model: &str,
    project: &str,
    tokens: &str,
    cost: &str,
    theme: Theme,
    header: bool,
) -> Div {
    div()
        .flex()
        .gap(px(18.))
        .items_center()
        .px(px(22.))
        .py(px(if header { 13. } else { 15. }))
        .border_t_1()
        .border_color(rgb(theme.line))
        .bg(rgb(if header { theme.bg } else { theme.panel }))
        .text_size(px(if header { 10. } else { 11. }))
        .text_color(rgb(if header { theme.muted } else { theme.ink }))
        .child(div().w(px(112.)).flex_shrink_0().child(time.to_owned()))
        .child(div().flex_1().min_w_0().truncate().child(model.to_owned()))
        .child(div().w(px(115.)).truncate().child(project.to_owned()))
        .child(
            div()
                .w(px(85.))
                .text_right()
                .font_family(mono_font())
                .child(tokens.to_owned()),
        )
        .child(div().w(px(85.)).text_right().child(cost.to_owned()))
}
fn record_totals(record: &UsageRecord, config: &Config) -> Totals {
    Totals {
        tokens: record.tokens.clone(),
        estimated: price_for(&record.model, &config.prices)
            .map(|price| price.cost(&record.tokens))
            .unwrap_or(0.0),
        ..Default::default()
    }
}
