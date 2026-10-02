#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod ui;
use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
gpui::actions!(open_usage, [Quit]);

fn main() {
    let demo = std::env::args().any(|arg| arg == "--demo");
    Application::new()
        .with_assets(ui::assets())
        .run(move |cx: &mut App| {
            cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
            cx.bind_keys([gpui::KeyBinding::new("secondary-q", Quit, None)]);
            #[cfg(target_os = "macos")]
            cx.set_menus(vec![gpui::Menu {
                name: "Open Usage".into(),
                items: vec![gpui::MenuItem::action("Quit Open Usage", Quit)],
            }]);
            let bounds = Bounds::centered(None, size(px(1340.), px(940.)), cx);
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
                    window.on_window_should_close(cx, |window, _| {
                        window.minimize_window();
                        false
                    });
                    cx.new(|cx| ui::Dashboard::new(window, cx, demo))
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
