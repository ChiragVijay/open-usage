use gpui::{App, Window};

/// Native window title. The Windows single-instance lookup uses this exact text.
pub const WINDOW_TITLE: &str = "Open Usage";

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
        use windows::Win32::UI::WindowsAndMessaging::ShowWindowAsync;
        unsafe {
            let _ = ShowWindowAsync(hwnd, visibility_command(hwnd));
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

/// Take the per-session window lock. Returns false when another real window
/// process already holds it; that window is brought forward and the caller exits.
///
/// Call this only after the per-user install handoff. The installer starts the
/// installed executable and exits first, so it never holds the lock.
#[cfg(windows)]
pub fn claim_single_instance() -> bool {
    use std::{thread, time::Duration};
    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, SetLastError, WIN32_ERROR,
            },
            System::Threading::CreateMutexW,
            UI::WindowsAndMessaging::FindWindowW,
        },
        core::{HSTRING, w},
    };

    let handle = unsafe {
        // CreateMutex leaves a stale last-error when it creates the object.
        SetLastError(WIN32_ERROR(0));
        match CreateMutexW(None, true, w!("Local\\dev.chirag.open-usage")) {
            Ok(handle) => handle,
            Err(_) => return true,
        }
    };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(handle) };
        // The first process takes the lock before its window exists.
        for _ in 0..50 {
            if let Ok(hwnd) =
                unsafe { FindWindowW(w!("Zed::Window"), &HSTRING::from(WINDOW_TITLE)) }
            {
                bring_forward(hwnd);
                return false;
            }
            thread::sleep(Duration::from_millis(100));
        }
        return false;
    }
    // HANDLE does not close on drop. Keep the kernel object until this
    // process exits so a later launch can find the window.
    let _held = Box::leak(Box::new(handle));
    true
}

#[cfg(target_os = "windows")]
fn bring_forward(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::{
        System::Threading::{AttachThreadInput, GetCurrentThreadId},
        UI::WindowsAndMessaging::{
            BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
            ShowWindow,
        },
    };
    unsafe {
        let _ = ShowWindow(hwnd, visibility_command(hwnd));
        let foreground = GetForegroundWindow();
        let current = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let attached_foreground = foreground_thread != 0
            && foreground_thread != current
            && AttachThreadInput(current, foreground_thread, true).as_bool();
        let attached_target = target_thread != 0
            && target_thread != current
            && target_thread != foreground_thread
            && AttachThreadInput(current, target_thread, true).as_bool();
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
        if attached_target {
            let _ = AttachThreadInput(current, target_thread, false);
        }
        if attached_foreground {
            let _ = AttachThreadInput(current, foreground_thread, false);
        }
    }
}

#[cfg(target_os = "windows")]
fn visibility_command(
    hwnd: windows::Win32::Foundation::HWND,
) -> windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD {
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, SW_RESTORE, SW_SHOW};
    if unsafe { IsIconic(hwnd).as_bool() } {
        SW_RESTORE
    } else {
        SW_SHOW
    }
}

#[cfg(test)]
mod tests {
    use super::WINDOW_TITLE;

    #[test]
    fn window_title_is_the_single_instance_lookup_text() {
        assert_eq!(WINDOW_TITLE, "Open Usage");
        assert!(WINDOW_TITLE.is_ascii());
    }
}
