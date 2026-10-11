#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod ui;
use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
gpui::actions!(open_usage, [Quit]);

fn main() {
    #[cfg(windows)]
    open_usage::install::install_and_handoff();
    Application::new()
        .with_assets(ui::assets())
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.bind_keys([gpui::KeyBinding::new("secondary-q", Quit, None)]);
            #[cfg(target_os = "macos")]
            cx.set_menus(vec![gpui::Menu {
                name: "Open Usage".into(),
                items: vec![gpui::MenuItem::action("Quit Open Usage", Quit)],
            }]);
            let bounds = Bounds::centered(None, size(px(1280.), px(860.)), cx);
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(420.), px(560.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Open Usage".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let dashboard = cx.new(|cx| ui::Dashboard::new(window, cx));
                    cx.new(|cx| gpui_component::Root::new(dashboard, window, cx))
                },
            );
            if result.is_err() {
                eprintln!("Open Usage could not open its native window");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
