use gpui::{App, Window};

/// Switch to a menu-bar app only after its tray icon is available.
pub fn configure_tray_app() {
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        let mtm = MainThreadMarker::new().expect("GPUI runs on the main thread");
        NSApplication::sharedApplication(mtm)
            .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

/// Hide the native window without minimizing it or destroying the dashboard.
pub fn hide(window: &Window, cx: &App) {
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = native_handle(window) {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindowAsync};
        // Queue visibility changes so native callbacks cannot re-enter GPUI's update.
        // Hiding removes the taskbar entry without changing the window's placement.
        unsafe {
            let _ = ShowWindowAsync(hwnd, SW_HIDE);
        }
    }
    #[cfg(target_os = "macos")]
    cx.hide();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    window.minimize_window();
    let _ = (window, cx);
}

/// Explicitly show a hidden window before asking GPUI to focus it.
pub fn show(window: &Window, cx: &App) {
    #[cfg(target_os = "windows")]
    if let Some(hwnd) = native_handle(window) {
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, SW_RESTORE, SW_SHOW, ShowWindowAsync,
        };
        unsafe {
            let command = if IsIconic(hwnd).as_bool() {
                SW_RESTORE
            } else {
                SW_SHOW
            };
            let _ = ShowWindowAsync(hwnd, command);
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        let mtm = MainThreadMarker::new().expect("GPUI runs on the main thread");
        NSApplication::sharedApplication(mtm).unhideWithoutActivation();
    }
    cx.activate(true);
    window.activate_window();
}

#[cfg(target_os = "windows")]
fn native_handle(window: &Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(windows::Win32::Foundation::HWND(
            handle.hwnd.get() as *mut std::ffi::c_void
        )),
        _ => None,
    }
}

/// Clear Windows' maximized/minimized placement before GPUI changes content size.
/// SetWindowPos alone retains that placement and restores the wrong size later.
pub fn restore_before_resize(window: &Window) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, IsZoomed, SW_RESTORE, ShowWindowAsync,
        };
        if let Some(hwnd) = native_handle(window) {
            // The handle belongs to this GPUI window and remains valid during this call.
            unsafe {
                if IsZoomed(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                    return ShowWindowAsync(hwnd, SW_RESTORE).as_bool();
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window;
    false
}
