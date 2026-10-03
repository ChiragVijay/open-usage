mod layout;
mod table;
mod theme;
mod tray;
mod views;
mod window;

use chrono::Utc;
use gpui::{div, prelude::*, px, rgb, *};
use gpui_component::input::{InputEvent, InputState};
use open_usage::{
    analytics::{self, Period},
    config::Config,
    model::{Provider, Snapshot},
    providers,
    store::Store,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
use table::{Column, CostFilter, Sort, Table};
use theme::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Activity,
    Models,
    Limits,
    Connections,
    Settings,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Accounts",
            Self::Activity => "Activity",
            Self::Models => "Models",
            Self::Limits => "Limits",
            Self::Connections => "Connections",
            Self::Settings => "Settings",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Activity => "activity",
            Self::Models => "models",
            Self::Limits => "limits",
            Self::Connections => "connections",
            Self::Settings => "settings",
        }
    }
}
#[derive(Clone)]
enum UiAction {
    Noop,
    Page(Page),
    Period(Period),
    Provider(Option<Provider>),
    AccountActivity(Provider),
    Sort(Table, Column),
    CostFilter(Table, CostFilter),
    ClearFilters(Table),
    Refresh,
    Export,
    Privacy,
    Dark,
    Compact,
    Quit,
    Metric(bool),
    AddHome(Provider),
    RemoveHome(Provider, usize),
    ImportCursor,
    ClearCursor,
    Interval(u64),
    OpenProvider(Provider),
    ActivityPage(usize),
    ActivityRows(Option<usize>),
    Detail(String),
    CloseDetail,
}

enum RefreshMsg {
    Update(Snapshot),
    Done(Result<Snapshot, String>),
}

pub struct Dashboard {
    config: Config,
    live: Snapshot,
    page: Page,
    period: Period,
    provider: Option<Provider>,
    token_chart: bool,
    hovered_day: Option<usize>,
    refreshing: bool,
    pending_refresh: bool,
    tray_attempted: bool,
    tray_error: Option<String>,
    last_minute: Instant,
    receiver: Receiver<RefreshMsg>,
    sender: Sender<RefreshMsg>,
    last_refresh: Instant,
    focus: FocusHandle,
    toast: Option<(String, Instant)>,
    tray: Option<tray::Tray>,
    compact: bool,
    pending_size: Option<Size<Pixels>>,
    activity_page: usize,
    model_query: Entity<InputState>,
    activity_query: Entity<InputState>,
    model_sort: Sort,
    activity_sort: Sort,
    model_cost: CostFilter,
    activity_cost: CostFilter,
    _input_subscriptions: Vec<Subscription>,
    /// `None` shows as many rows as fit in the window.
    activity_rows: Option<usize>,
    detail: Option<String>,
}

impl Dashboard {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        open_usage::pricing::start();
        sync_components(config.dark, window, cx);
        let focus = cx.focus_handle();
        window.focus(&focus);
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            if weak
                .update(cx, |view, _| view.tray.is_some())
                .unwrap_or(false)
            {
                window.minimize_window();
                false
            } else {
                cx.quit();
                true
            }
        });
        let model_query = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search models or accounts")
                .clean_on_escape()
        });
        let activity_query = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search models, accounts or projects")
                .clean_on_escape()
        });
        let input_subscriptions = vec![
            cx.subscribe(&model_query, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&activity_query, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.activity_page = 0;
                    cx.notify();
                }
            }),
        ];
        let mut view = Self {
            config,
            live,
            page: Page::Overview,
            period: Period::Month,
            provider: None,
            token_chart: true,
            hovered_day: None,
            refreshing: false,
            pending_refresh: false,
            tray_attempted: false,
            tray_error: None,
            last_minute: Instant::now(),
            receiver,
            sender,
            last_refresh: Instant::now(),
            focus,
            toast: error.map(|error| (error, Instant::now())),
            tray: None,
            compact: false,
            pending_size: None,
            activity_page: 0,
            model_query,
            activity_query,
            model_sort: Sort::new(Column::Tokens),
            activity_sort: Sort::new(Column::Time),
            model_cost: CostFilter::All,
            activity_cost: CostFilter::All,
            _input_subscriptions: input_subscriptions,
            activity_rows: None,
            detail: None,
        };
        view.refresh(cx);
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
        &self.live
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
            let sender_done = sender.clone();
            let result = std::panic::catch_unwind(|| {
                providers::collect_with(&config, &previous, |snapshot| {
                    let _ = sender.send(RefreshMsg::Update(snapshot.clone()));
                })
            });
            let result = match result {
                Ok(result) => result.map_err(|error| error.to_string()),
                Err(_) => Err("Usage scan stopped unexpectedly. Refresh to retry.".into()),
            };
            let _ = sender_done.send(RefreshMsg::Done(result));
        });
        cx.notify();
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(size) = self.pending_size.take() {
            window.resize(size);
            cx.notify();
        }
        if !self.tray_attempted {
            self.tray_attempted = true;
            match tray::create() {
                Ok(tray) => {
                    self.tray = Some(tray);
                    self.update_tray();
                }
                Err(error) => {
                    self.tray_error = Some(error.to_string());
                    self.message(
                        "System tray could not be initialized. Close the window to quit.",
                        cx,
                    );
                }
            }
        }
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                RefreshMsg::Update(snapshot) => self.live = snapshot,
                RefreshMsg::Done(result) => {
                    self.refreshing = false;
                    match result {
                        Ok(snapshot) => self.live = snapshot,
                        Err(error) => self.message(error, cx),
                    }
                    if self.pending_refresh {
                        self.pending_refresh = false;
                        self.refresh(cx);
                    }
                }
            }
            self.update_tray();
            cx.notify();
        }
        if open_usage::pricing::take_updated() {
            cx.notify();
        }
        if self.last_minute.elapsed() >= Duration::from_secs(60) {
            self.last_minute = Instant::now();
            self.update_tray();
            cx.notify();
        }
        if !self.refreshing
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
                    if matches!(action, UiAction::Compact) {
                        self.compact = false;
                    }
                    let show_window = matches!(action, UiAction::Page(_) | UiAction::Compact);
                    let quit = matches!(action, UiAction::Quit);
                    self.perform(action, window, cx);
                    if quit {
                        return;
                    }
                    if show_window {
                        window.activate_window();
                        cx.activate(true);
                    }
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
                self.resize_window(window, size(px(440.), px(700.)));
                window.focus(&self.focus);
                window.activate_window();
                cx.activate(true);
                cx.notify();
            }
        }
    }
    fn visible_accounts(&self) -> Vec<&open_usage::model::ProviderStatus> {
        self.data()
            .providers
            .iter()
            .filter(|status| {
                self.config.enabled[status.provider.index()]
                    && (status.detected
                        || status.connected
                        || self
                            .data()
                            .records
                            .iter()
                            .any(|record| record.provider == status.provider))
            })
            .collect()
    }
    fn resize_window(&mut self, window: &mut Window, size: Size<Pixels>) {
        self.pending_size = None;
        if window::restore_before_resize(window) {
            // Windows restores asynchronously. Resize after its placement has settled.
            self.pending_size = Some(size);
        } else {
            window.resize(size);
        }
    }
    fn spend(&self, totals: &analytics::Totals) -> String {
        totals
            .spend()
            .map(|cost| self.money(cost))
            .unwrap_or_else(|| "—".into())
    }
    fn cost_caption(&self, totals: &analytics::Totals) -> String {
        if totals.events == 0 {
            return "No usage".into();
        }
        if totals.reported_rows + totals.estimated_rows > 0 {
            let caption = match (totals.reported_rows, totals.estimated_rows) {
                (_, 0) => "Reported",
                (0, _) => "Estimated · API rates",
                _ => "Reported + estimated",
            };
            return if totals.events > totals.reported_rows + totals.estimated_rows {
                match (totals.reported_rows, totals.estimated_rows) {
                    (_, 0) => "Partial · reported".into(),
                    (0, _) => "Partial · estimated".into(),
                    _ => "Partial · reported + estimated".into(),
                }
            } else {
                caption.into()
            };
        }
        match open_usage::pricing::state() {
            open_usage::pricing::RateState::Loading => "Loading model rates".into(),
            open_usage::pricing::RateState::Unavailable => "Model rates unavailable".into(),
            open_usage::pricing::RateState::Ready => "No public rate".into(),
        }
    }
    fn event_cost(&self, record: &open_usage::model::UsageRecord) -> (String, &'static str) {
        match analytics::record_cost(record) {
            Some((cost, false)) => (self.money(cost), "Reported"),
            Some((cost, true)) => (self.money(cost), "Estimated · API rate"),
            None => ("—".into(), "No public rate"),
        }
    }
    fn update_tray(&self) {
        if let Some(tray) = &self.tray {
            let analysis =
                analytics::analyze(self.data(), &self.config, Period::Today, None, Utc::now());
            let mut tooltip = format!(
                "Open Usage\n{} tokens today",
                self.number(analysis.totals.tokens.total())
            );
            for status in self.visible_accounts() {
                if let Some(limit) = status.limits.first() {
                    let remaining = if self.config.privacy {
                        "Hidden".into()
                    } else if limit.expired(Utc::now()) {
                        "Refresh needed".into()
                    } else {
                        format!(
                            "{:.0}% left{}",
                            limit.remaining(),
                            if status.stale { " (cached)" } else { "" }
                        )
                    };
                    tooltip.push_str(&format!("\n{}: {remaining}", status.provider.name()));
                }
            }
            let _ = tray
                .icon
                .set_tooltip(Some(tooltip.chars().take(120).collect::<String>()));
            #[cfg(target_os = "macos")]
            {
                let title = if self.config.privacy {
                    "Open Usage".into()
                } else {
                    self.visible_accounts()
                        .iter()
                        .filter_map(|status| {
                            status
                                .limits
                                .first()
                                .filter(|limit| !limit.expired(Utc::now()))
                                .map(|limit| {
                                    format!(
                                        "{} {:.0}%{}",
                                        status.provider.name(),
                                        limit.remaining(),
                                        if status.stale { "~" } else { "" }
                                    )
                                })
                        })
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                tray.icon.set_title(Some(if title.is_empty() {
                    "Open Usage".into()
                } else {
                    title
                }));
            }
        }
    }
    fn perform(&mut self, action: UiAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            UiAction::Noop => {}
            UiAction::Page(page) => {
                self.page = page;
                if page == Page::Overview {
                    self.provider = None;
                }
                self.detail = None;
                if self.compact {
                    self.compact = false;
                    self.resize_window(window, size(px(1280.), px(860.)));
                }
                window.focus(&self.focus);
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
            UiAction::AccountActivity(provider) => {
                self.provider = Some(provider);
                self.page = Page::Activity;
                self.activity_page = 0;
                self.activity_cost = CostFilter::All;
                self.activity_query
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.detail = None;
                window.focus(&self.focus);
            }
            UiAction::Sort(table, column) => match table {
                Table::Models => self.model_sort.select(column),
                Table::Activity => {
                    self.activity_sort.select(column);
                    self.activity_page = 0;
                }
            },
            UiAction::CostFilter(table, filter) => match table {
                Table::Models => self.model_cost = filter,
                Table::Activity => {
                    self.activity_cost = filter;
                    self.activity_page = 0;
                }
            },
            UiAction::ClearFilters(table) => {
                self.provider = None;
                match table {
                    Table::Models => {
                        self.model_cost = CostFilter::All;
                        self.model_query
                            .update(cx, |input, cx| input.set_value("", window, cx));
                    }
                    Table::Activity => {
                        self.activity_cost = CostFilter::All;
                        self.activity_page = 0;
                        self.activity_query
                            .update(cx, |input, cx| input.set_value("", window, cx));
                    }
                }
            }
            UiAction::Export => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_file_name("usage.csv")
                    .save_file()
                {
                    let filtered = (self.page == Page::Activity).then(|| Snapshot {
                        records: self.activity_records(cx).into_iter().cloned().collect(),
                        ..Default::default()
                    });
                    match analytics::export_csv(
                        &path,
                        filtered.as_ref().unwrap_or(self.data()),
                        &self.config,
                        self.period,
                        self.provider,
                    ) {
                        Ok(count) => self.message(format!("Exported {count} usage records"), cx),
                        Err(error) => self.message(error.to_string(), cx),
                    }
                }
            }
            UiAction::Privacy => {
                self.config.privacy = !self.config.privacy;
                if self.config.privacy {
                    self.activity_query
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                self.save(cx);
                self.update_tray();
            }
            UiAction::Dark => {
                self.config.dark = !self.config.dark;
                sync_components(self.config.dark, window, cx);
                self.save(cx);
            }
            UiAction::Compact => {
                self.compact = !self.compact;
                let size = if self.compact {
                    size(px(440.), px(700.))
                } else {
                    size(px(1280.), px(860.))
                };
                self.resize_window(window, size);
                window.focus(&self.focus);
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
            UiAction::RemoveHome(provider, index) => {
                let paths = &mut self.config.custom_homes[provider.index()];
                if index < paths.len() {
                    paths.remove(index);
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
            UiAction::Interval(seconds) => {
                self.config.refresh_seconds = seconds;
                self.save(cx);
            }
            UiAction::OpenProvider(provider) => {
                let url = match provider {
                    Provider::Codex => "https://chatgpt.com/codex/settings/usage",
                    Provider::Claude => "https://claude.ai/settings/usage",
                    Provider::Cursor => "https://cursor.com/dashboard?tab=usage",
                    Provider::Kiro => "https://app.kiro.dev/",
                    Provider::Antigravity => "https://antigravity.google/",
                    Provider::OpenCode => "https://opencode.ai/zen",
                };
                if open::that(url).is_err() {
                    self.message("The provider dashboard could not be opened", cx);
                }
            }
            UiAction::ActivityPage(page) => self.activity_page = page,
            UiAction::ActivityRows(limit) => {
                self.activity_rows = limit;
                self.activity_page = 0;
            }
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
    ) -> gpui_component::button::Button {
        use gpui_component::{
            Sizable,
            button::{Button, ButtonCustomVariant, ButtonVariants},
        };
        let theme = self.theme();
        Button::new(SharedString::from(id.into()))
            .label(text.into())
            .small()
            .text_size(px(12.))
            .h(px(28.))
            .rounded(px(5.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(rgb(if active { theme.wash } else { theme.panel }).into())
                    .foreground(rgb(if active { theme.accent } else { theme.ink }).into())
                    .border(rgb(if active { theme.wash } else { theme.line }).into())
                    .hover(rgb(theme.wash).into())
                    .active(rgb(theme.wash).into()),
            )
            .when_some(glyph, |button, glyph| {
                button.icon(gpui_component::Icon::default().path(glyph))
            })
            .on_click(
                cx.listener(move |view, _, window, cx| view.perform(action.clone(), window, cx)),
            )
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        let typing = self
            .model_query
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
            || self
                .activity_query
                .read(cx)
                .focus_handle(cx)
                .is_focused(window);
        if typing && !mods.control && !mods.platform {
            return;
        }
        let action = if (mods.control || mods.platform) && mods.shift {
            match key {
                "1" => Some(UiAction::Period(Period::TwoHours)),
                "2" => Some(UiAction::Period(Period::Day)),
                "3" => Some(UiAction::Period(Period::Week)),
                "4" => Some(UiAction::Period(Period::Month)),
                "5" => Some(UiAction::Period(Period::Quarter)),
                _ => None,
            }
        } else if mods.control || mods.platform {
            match key {
                "r" => Some(UiAction::Refresh),
                "p" => Some(UiAction::Privacy),
                "q" => Some(UiAction::Quit),
                "1" => Some(UiAction::Page(Page::Overview)),
                "2" => Some(UiAction::Page(Page::Models)),
                "3" => Some(UiAction::Page(Page::Limits)),
                "4" => Some(UiAction::Page(Page::Activity)),
                "5" => Some(UiAction::Page(Page::Connections)),
                "6" => Some(UiAction::Page(Page::Settings)),
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
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width);
        let height = f32::from(viewport.height);
        let rail = width < 1000.;
        let content = if self.compact {
            self.compact_view(cx).into_any_element()
        } else {
            let page = match self.page {
                Page::Overview => self.overview(width, height, cx),
                Page::Activity => self.activity(width, height, cx),
                Page::Models => self.models_view(width, cx),
                Page::Limits => self.limits_view(width, cx),
                Page::Connections => self.connections(cx),
                Page::Settings => self.settings(cx),
            };
            div()
                .flex()
                .size_full()
                .child(self.sidebar(rail, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(self.header(width < 900., cx))
                        .child({
                            let pinned =
                                matches!(self.page, Page::Overview | Page::Activity | Page::Models);
                            div()
                                .id(SharedString::from(format!(
                                    "main-scroll-{}",
                                    self.page.title()
                                )))
                                .flex_1()
                                .min_h_0()
                                .p(px(layout::page_padding(width, height)))
                                .when(pinned, |region| region.flex().flex_col().overflow_hidden())
                                .when(!pinned, |region| region.overflow_y_scroll())
                                .child(page)
                        }),
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
            .text_size(px(14.))
            .line_height(relative(1.4))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| {
                    if !view.focus.contains_focused(window, cx) {
                        window.focus(&view.focus);
                    }
                }),
            )
            .on_key_down(cx.listener(Self::key))
            .child(content)
            .when_some(self.toast.as_ref(), |root, (message, _)| {
                root.child(
                    div()
                        .absolute()
                        .bottom(px(22.))
                        .right(px(24.))
                        .max_w(px(520.))
                        .rounded(px(6.))
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
