mod theme;
mod tray;
mod views;

use chrono::Utc;
use gpui::{div, prelude::*, px, rgb, *};
use open_usage::{
    analytics::{self, Period},
    config::Config,
    demo,
    model::{Provider, Snapshot},
    providers,
    store::Store,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
use theme::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Activity,
    Limits,
    Connections,
    Settings,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Activity => "Activity",
            Self::Limits => "Limits",
            Self::Connections => "Connections",
            Self::Settings => "Settings",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Activity => "activity",
            Self::Limits => "limits",
            Self::Connections => "connections",
            Self::Settings => "settings",
        }
    }
}
#[derive(Clone)]
enum UiAction {
    Page(Page),
    Period(Period),
    Provider(Option<Provider>),
    Refresh,
    Export,
    Online,
    Privacy,
    Dark,
    Demo,
    Compact,
    Quit,
    Metric(bool),
    AddHome(Provider),
    ImportCursor,
    ClearCursor,
    Enable(Provider),
    ImportPrices,
    ResetPrices,
    PriceTemplate,
    Budget(f64),
    Interval(u64),
    OpenProvider(Provider),
    NextActivity,
    PreviousActivity,
    Detail(String),
    CloseDetail,
}

pub struct Dashboard {
    config: Config,
    live: Snapshot,
    example: Snapshot,
    demo: bool,
    page: Page,
    period: Period,
    provider: Option<Provider>,
    token_chart: bool,
    hovered_day: Option<usize>,
    refreshing: bool,
    pending_refresh: bool,
    tray_attempted: bool,
    last_minute: Instant,
    receiver: Receiver<Result<Snapshot, String>>,
    sender: Sender<Result<Snapshot, String>>,
    last_refresh: Instant,
    focus: FocusHandle,
    toast: Option<(String, Instant)>,
    tray: Option<tray::Tray>,
    compact: bool,
    activity_page: usize,
    detail: Option<String>,
}

impl Dashboard {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, demo: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let (config, error) = match Config::load() {
            Ok(config) => (config, None),
            Err(error) => (Config::default(), Some(error.to_string())),
        };
        let mut live = Store::open()
            .and_then(|store| store.load())
            .ok()
            .flatten()
            .unwrap_or_default();
        for provider in &mut live.providers {
            provider.stale = true;
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut view = Self {
            config,
            live,
            example: demo::snapshot(),
            demo,
            page: Page::Overview,
            period: Period::Month,
            provider: None,
            token_chart: false,
            hovered_day: None,
            refreshing: false,
            pending_refresh: false,
            tray_attempted: false,
            last_minute: Instant::now(),
            receiver,
            sender,
            last_refresh: Instant::now(),
            focus,
            toast: error.map(|error| (error, Instant::now())),
            tray: None,
            compact: false,
            activity_page: 0,
            detail: None,
        };
        if !demo {
            view.refresh(cx);
        }
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if cx
                    .update(|window, cx| view.update(cx, |view, cx| view.tick(window, cx)))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        view
    }
    fn data(&self) -> &Snapshot {
        if self.demo { &self.example } else { &self.live }
    }
    fn theme(&self) -> Theme {
        Theme::new(self.config.dark)
    }
    fn money(&self, value: f64) -> String {
        if self.config.privacy {
            "••••".into()
        } else {
            format!("${value:.2}")
        }
    }
    fn number(&self, value: u64) -> String {
        if self.config.privacy {
            "••••".into()
        } else {
            analytics::compact(value)
        }
    }
    fn message(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.toast = Some((text.into(), Instant::now()));
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.config.save() {
            self.message(error.to_string(), cx);
        }
        cx.notify();
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.example = demo::snapshot();
            cx.notify();
            return;
        }
        if self.refreshing {
            self.pending_refresh = true;
            return;
        }
        self.refreshing = true;
        self.last_refresh = Instant::now();
        let config = self.config.clone();
        let previous = self.live.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| providers::collect(&config, &previous));
            let result = match result {
                Ok(result) => result.map_err(|error| error.to_string()),
                Err(_) => Err("Usage scan stopped unexpectedly. Refresh to retry.".into()),
            };
            let _ = sender.send(result);
        });
        cx.notify();
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tray_attempted {
            self.tray_attempted = true;
            if let Ok(tray) = tray::create() {
                self.tray = Some(tray)
            }
        }
        while let Ok(result) = self.receiver.try_recv() {
            self.refreshing = false;
            match result {
                Ok(snapshot) => self.live = snapshot,
                Err(error) => self.message(error, cx),
            }
            self.update_tray();
            cx.notify();
            if self.pending_refresh {
                self.pending_refresh = false;
                self.refresh(cx);
            }
        }
        if self.last_minute.elapsed() >= Duration::from_secs(60) {
            self.last_minute = Instant::now();
            self.update_tray();
            cx.notify();
        }
        if !self.demo
            && !self.refreshing
            && self.last_refresh.elapsed() >= Duration::from_secs(self.config.refresh_seconds)
        {
            self.refresh(cx);
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, time)| time.elapsed() > Duration::from_secs(12))
        {
            self.toast = None;
            cx.notify();
        }
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if let Some(tray) = &self.tray {
                let action = if event.id == tray.overview {
                    Some(UiAction::Page(Page::Overview))
                } else if event.id == tray.compact {
                    Some(UiAction::Compact)
                } else if event.id == tray.refresh {
                    Some(UiAction::Refresh)
                } else if event.id == tray.quit {
                    Some(UiAction::Quit)
                } else {
                    None
                };
                if let Some(action) = action {
                    self.perform(action, window, cx);
                    window.activate_window();
                    cx.activate(true);
                }
            }
        }
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            ) {
                self.compact = true;
                window.resize(size(px(460.), px(680.)));
                window.activate_window();
                cx.activate(true);
                cx.notify();
            }
        }
    }
    fn update_tray(&self) {
        if let Some(tray) = &self.tray {
            let analysis =
                analytics::analyze(self.data(), &self.config, Period::Today, None, Utc::now());
            let mut tooltip = format!(
                "Open Usage{} · Today {} estimated · {} tokens",
                if self.demo { " · Example data" } else { "" },
                self.money(analysis.totals.estimated),
                self.number(analysis.totals.tokens.total())
            );
            for status in &self.data().providers {
                if let Some(limit) = status.limits.first() {
                    tooltip.push_str(&format!(
                        "\n{}: {}",
                        status.provider.name(),
                        if self.config.privacy {
                            "••••".into()
                        } else {
                            format!(
                                "{:.0}% left{}",
                                limit.remaining(),
                                if status.stale { " (cached)" } else { "" }
                            )
                        }
                    ));
                }
            }
            let _ = tray.icon.set_tooltip(Some(tooltip));
            #[cfg(target_os = "macos")]
            tray.icon
                .set_title(Some(self.money(analysis.totals.estimated)));
        }
    }
    fn perform(&mut self, action: UiAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            UiAction::Page(page) => {
                self.page = page;
                self.detail = None;
                if self.compact {
                    self.compact = false;
                    window.resize(size(px(1340.), px(940.)));
                }
            }
            UiAction::Period(period) => {
                self.period = period;
                self.activity_page = 0;
                self.hovered_day = None;
            }
            UiAction::Provider(provider) => {
                self.provider = provider;
                self.activity_page = 0;
                self.hovered_day = None;
            }
            UiAction::Refresh => self.refresh(cx),
            UiAction::Export => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_file_name(if self.demo {
                        "example-usage.csv"
                    } else {
                        "usage.csv"
                    })
                    .save_file()
                {
                    match analytics::export_csv(
                        &path,
                        self.data(),
                        &self.config,
                        self.period,
                        self.provider,
                    ) {
                        Ok(count) => self.message(format!("Exported {count} usage records"), cx),
                        Err(error) => self.message(error.to_string(), cx),
                    }
                }
            }
            UiAction::Online => {
                self.config.online = !self.config.online;
                self.save(cx);
                self.refresh(cx);
            }
            UiAction::Privacy => {
                self.config.privacy = !self.config.privacy;
                self.save(cx);
                self.update_tray();
            }
            UiAction::Dark => {
                self.config.dark = !self.config.dark;
                self.save(cx);
            }
            UiAction::Demo => {
                self.demo = !self.demo;
                self.detail = None;
                self.activity_page = 0;
                if !self.demo {
                    self.refresh(cx);
                }
                self.update_tray();
            }
            UiAction::Compact => {
                self.compact = !self.compact;
                window.resize(if self.compact {
                    size(px(460.), px(680.))
                } else {
                    size(px(1340.), px(940.))
                });
            }
            UiAction::Quit => cx.quit(),
            UiAction::Metric(tokens) => {
                self.token_chart = tokens;
                self.hovered_day = None;
            }
            UiAction::AddHome(provider) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title(format!("Choose {} data folder", provider.name()))
                    .pick_folder()
                {
                    let paths = &mut self.config.custom_homes[provider.index()];
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                    self.save(cx);
                    self.refresh(cx);
                }
            }
            UiAction::ImportCursor => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Cursor usage export", &["csv"])
                    .pick_file()
                {
                    match providers::history::read_cursor_csv(&path) {
                        Ok(records) => {
                            self.config.cursor_imports = vec![path];
                            self.save(cx);
                            self.refresh(cx);
                            self.message(format!("Imported {} Cursor events", records.len()), cx);
                        }
                        Err(error) => self.message(error.to_string(), cx),
                    }
                }
            }
            UiAction::ClearCursor => {
                self.config.cursor_imports.clear();
                self.save(cx);
                self.refresh(cx);
            }
            UiAction::Enable(provider) => {
                self.config.enabled[provider.index()] = !self.config.enabled[provider.index()];
                self.save(cx);
                self.refresh(cx);
            }
            UiAction::ImportPrices => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Model prices", &["csv"])
                    .pick_file()
                {
                    match open_usage::pricing::import_prices(&path) {
                        Ok(prices) => {
                            self.config.prices.extend(prices);
                            self.save(cx);
                            self.message("Model prices updated for all history", cx);
                        }
                        Err(error) => self.message(error.to_string(), cx),
                    }
                }
            }
            UiAction::ResetPrices => {
                self.config.prices.clear();
                self.save(cx);
                self.message("Restored baseline model prices", cx);
            }
            UiAction::PriceTemplate => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_file_name("model-prices.csv")
                    .save_file()
                {
                    match std::fs::write(
                        path,
                        "model,input,output,cache_read,cache_write\ngpt-5.4,2.5,15,0.25,2.5\nclaude-sonnet-4-6,3,15,0.30,3.75\n",
                    ) {
                        Ok(_) => self.message("Saved price template · USD per million tokens", cx),
                        Err(error) => self.message(error.to_string(), cx),
                    }
                }
            }
            UiAction::Budget(budget) => {
                self.config.monthly_budget = budget;
                self.save(cx);
            }
            UiAction::Interval(seconds) => {
                self.config.refresh_seconds = seconds;
                self.save(cx);
            }
            UiAction::OpenProvider(provider) => {
                let url = match provider {
                    Provider::Codex => "https://chatgpt.com/codex/settings/usage",
                    Provider::Claude => "https://claude.ai/settings/usage",
                    Provider::Cursor => "https://cursor.com/dashboard?tab=usage",
                };
                if open::that(url).is_err() {
                    self.message("The provider dashboard could not be opened", cx);
                }
            }
            UiAction::NextActivity => self.activity_page += 1,
            UiAction::PreviousActivity => self.activity_page = self.activity_page.saturating_sub(1),
            UiAction::Detail(id) => self.detail = Some(id),
            UiAction::CloseDetail => self.detail = None,
        }
        cx.notify();
    }
    fn button(
        &self,
        id: impl Into<String>,
        text: impl Into<String>,
        glyph: Option<&'static str>,
        active: bool,
        action: UiAction,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        let key_action = action.clone();
        div()
            .id(SharedString::from(id.into()))
            .focusable()
            .tab_index(0)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(7.))
            .h(px(34.))
            .px(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(if active { theme.wash } else { theme.line }))
            .bg(rgb(if active { theme.wash } else { theme.panel }))
            .text_color(rgb(if active { theme.accent } else { theme.ink }))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(theme.wash)))
            .focus(move |style| style.border_color(rgb(theme.accent)))
            .when_some(glyph, |button, glyph| button.child(icon(glyph, theme)))
            .child(text.into())
            .on_click(
                cx.listener(move |view, _, window, cx| view.perform(action.clone(), window, cx)),
            )
            .on_key_down(cx.listener(move |view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    view.perform(key_action.clone(), window, cx);
                    cx.stop_propagation();
                }
            }))
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        let action = if mods.control || mods.platform {
            match key {
                "r" => Some(UiAction::Refresh),
                "p" => Some(UiAction::Privacy),
                "q" => Some(UiAction::Quit),
                "1" => Some(UiAction::Page(Page::Overview)),
                "2" => Some(UiAction::Page(Page::Activity)),
                "3" => Some(UiAction::Page(Page::Limits)),
                "4" => Some(UiAction::Page(Page::Connections)),
                "5" => Some(UiAction::Page(Page::Settings)),
                _ => None,
            }
        } else {
            match key {
                "c" => Some(UiAction::Metric(false)),
                "t" => Some(UiAction::Metric(true)),
                "l" => Some(UiAction::Page(Page::Limits)),
                "escape" => Some(UiAction::CloseDetail),
                "tab" => {
                    if mods.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    None
                }
                _ => None,
            }
        };
        if let Some(action) = action {
            self.perform(action, window, cx);
            cx.stop_propagation();
        }
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme();
        let content = if self.compact || window.viewport_size().width < px(850.) {
            self.compact_view(cx).into_any_element()
        } else {
            let page = match self.page {
                Page::Overview => self.overview(cx),
                Page::Activity => self.activity(cx),
                Page::Limits => self.limits_view(cx),
                Page::Connections => self.connections(cx),
                Page::Settings => self.settings(cx),
            };
            div()
                .flex()
                .size_full()
                .child(self.sidebar(cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(self.header(cx))
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "main-scroll-{}",
                                    self.page.title()
                                )))
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .p(px(30.))
                                .child(page),
                        ),
                )
                .into_any_element()
        };
        div()
            .id("root")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(rgb(theme.bg))
            .text_color(rgb(theme.ink))
            .font_family(body_font())
            .text_size(px(13.))
            .on_key_down(cx.listener(Self::key))
            .child(content)
            .when_some(self.toast.as_ref(), |root, (message, _)| {
                root.child(
                    div()
                        .absolute()
                        .bottom(px(22.))
                        .right(px(24.))
                        .max_w(px(520.))
                        .rounded(px(12.))
                        .bg(rgb(theme.ink))
                        .text_color(rgb(theme.panel))
                        .p(px(14.))
                        .shadow_lg()
                        .child(message.clone()),
                )
            })
    }
}
pub fn assets() -> Assets {
    Assets
}
