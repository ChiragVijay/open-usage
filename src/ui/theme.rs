use gpui::{Div, FontWeight, SharedString, Svg, div, prelude::*, px, rgb};
use open_usage::model::Provider;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: u32,
    pub panel: u32,
    pub sidebar: u32,
    pub ink: u32,
    pub muted: u32,
    pub line: u32,
    pub wash: u32,
    pub accent: u32,
}
impl Theme {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                bg: 0x191b1f,
                panel: 0x21242a,
                sidebar: 0x202227,
                ink: 0xf1f3f5,
                muted: 0xa6abb4,
                line: 0x353a43,
                wash: 0x30323e,
                accent: 0xf1f3f5,
            }
        } else {
            Self {
                bg: 0xffffff,
                panel: 0xffffff,
                sidebar: 0xf7f8fa,
                ink: 0x24262b,
                muted: 0x68707c,
                line: 0xe5e8ed,
                wash: 0xefeff6,
                accent: 0x24262b,
            }
        }
    }
}
/// Series colors from Notion's tag palette, so charts stay distinct without shouting.
pub fn color(provider: Provider, dark: bool) -> u32 {
    match (provider, dark) {
        (Provider::Codex, false) => 0x337ea9,
        (Provider::Claude, false) => 0xd9730d,
        (Provider::Cursor, false) => 0x9065b0,
        (Provider::Codex, true) => 0x529cca,
        (Provider::Claude, true) => 0xe08a3c,
        (Provider::Cursor, true) => 0xa484c7,
        (Provider::Kiro, false) => 0xad4381,
        (Provider::Kiro, true) => 0xd779b0,
        (Provider::Antigravity, false) => 0x268269,
        (Provider::Antigravity, true) => 0x53b99d,
        (Provider::OpenCode, false) => 0x68707c,
        (Provider::OpenCode, true) => 0xb4bcc8,
    }
}
pub fn tint(provider: Provider, dark: bool) -> u32 {
    match (provider, dark) {
        (Provider::Codex, false) => 0xe7f3f8,
        (Provider::Claude, false) => 0xfadec9,
        (Provider::Cursor, false) => 0xe8deee,
        (Provider::Codex, true) => 0x1c3a4c,
        (Provider::Claude, true) => 0x4a3214,
        (Provider::Cursor, true) => 0x3c2e4a,
        (Provider::Kiro, false) => 0xf8e6f0,
        (Provider::Kiro, true) => 0x482338,
        (Provider::Antigravity, false) => 0xe1f3ed,
        (Provider::Antigravity, true) => 0x193d32,
        (Provider::OpenCode, false) => 0xeceef1,
        (Provider::OpenCode, true) => 0x323944,
    }
}
pub fn body_font() -> &'static str {
    if cfg!(target_os = "windows") {
        "Segoe UI"
    } else {
        ".AppleSystemUIFont"
    }
}
pub fn display_font() -> &'static str {
    if cfg!(target_os = "windows") {
        "Segoe UI Variable Display"
    } else {
        ".AppleSystemUIFont"
    }
}
pub fn mono_font() -> &'static str {
    if cfg!(target_os = "windows") {
        "Cascadia Mono"
    } else {
        "Menlo"
    }
}
pub fn sync_components(dark: bool, window: &mut gpui::Window, cx: &mut gpui::App) {
    gpui_component::Theme::change(
        if dark {
            gpui_component::ThemeMode::Dark
        } else {
            gpui_component::ThemeMode::Light
        },
        Some(window),
        cx,
    );
    let theme = gpui_component::Theme::global_mut(cx);
    theme.font_family = body_font().into();
    theme.font_size = px(14.);
    theme.mono_font_family = mono_font().into();
    theme.radius = px(4.);
    theme.radius_lg = px(6.);
    theme.shadow = false;
}
pub fn label(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .text_size(px(13.))
        .text_color(rgb(theme.muted))
        .child(text.into())
}
pub fn title(text: impl Into<SharedString>, theme: Theme) -> Div {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme.ink))
        .child(text.into())
}
pub fn panel(theme: Theme) -> Div {
    div()
        .bg(rgb(theme.panel))
        .border_1()
        .border_color(rgb(theme.line))
        .rounded(px(8.))
        .p(px(18.))
}
pub fn dot(provider: Provider, dark: bool) -> Div {
    div()
        .size(px(7.))
        .rounded_full()
        .bg(rgb(color(provider, dark)))
        .flex_shrink_0()
}
pub fn icon(name: &'static str, theme: Theme) -> Svg {
    gpui::svg()
        .path(name)
        .size(px(18.))
        .text_color(rgb(theme.muted))
}
pub fn provider_mark(provider: Provider, dark: bool) -> Div {
    div()
        .size(px(32.))
        .rounded(px(4.))
        .bg(rgb(tint(provider, dark)))
        .flex()
        .items_center()
        .justify_center()
        .child(
            gpui::svg()
                .path(provider.key())
                .size(px(22.))
                .text_color(rgb(color(provider, dark))),
        )
}

pub struct Assets;
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let provider_logo: Option<&'static [u8]> = match path {
            "codex" => Some(include_bytes!("../../assets/providers/codex.svg")),
            "claude" => Some(include_bytes!("../../assets/providers/claude.svg")),
            "cursor" => Some(include_bytes!("../../assets/providers/cursor.svg")),
            "kiro" => Some(include_bytes!("../../assets/providers/kiro.svg")),
            "antigravity" => Some(include_bytes!("../../assets/providers/antigravity.svg")),
            "opencode" => Some(include_bytes!("../../assets/providers/opencode.svg")),
            _ => None,
        };
        if let Some(logo) = provider_logo {
            return Ok(Some(std::borrow::Cow::Borrowed(logo)));
        }
        let shape = match path {
            "overview" => {
                "<rect x='3' y='3' width='7' height='7' rx='1.5'/><rect x='14' y='3' width='7' height='7' rx='1.5'/><rect x='3' y='14' width='7' height='7' rx='1.5'/><rect x='14' y='14' width='7' height='7' rx='1.5'/>"
            }
            "activity" => "<path d='M3 12h4l3-7 4 14 3-7h4'/>",
            "models" => "<path d='m12 3 9 5-9 5-9-5zM3 12l9 5 9-5M3 16l9 5 9-5'/>",
            "limits" => "<circle cx='12' cy='12' r='9'/><path d='M12 7v5l4 2'/>",
            "connections" => "<path d='M8 3v5m8-5v5M6 8h12v3a6 6 0 0 1-12 0zM12 17v4'/>",
            "settings" => {
                "<path d='M4 7h16M4 17h16'/><circle cx='9' cy='7' r='3' fill='white'/><circle cx='15' cy='17' r='3' fill='white'/>"
            }
            "refresh" => "<path d='M20 8a8 8 0 1 0 0 8M20 3v5h-5'/>",
            "export" => "<path d='M12 3v12m-4-4 4 4 4-4M4 15v5h16v-5'/>",
            "privacy" => {
                "<path d='M3 12s3-6 9-6 9 6 9 6-3 6-9 6-9-6-9-6'/><circle cx='12' cy='12' r='3'/>"
            }
            "theme" => "<path d='M20 15A9 9 0 0 1 9 4a9 9 0 1 0 11 11z'/>",
            "arrow" => "<path d='m9 5 7 7-7 7'/>",
            "brand" => "<path d='M4 16V8a4 4 0 0 1 8 0v8a4 4 0 0 0 8 0V8M4 12h16'/>",
            _ => return Ok(None),
        };
        Ok(Some(std::borrow::Cow::Owned(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='1.7' stroke-linecap='round' stroke-linejoin='round'>{shape}</svg>").into_bytes())))
    }
    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
