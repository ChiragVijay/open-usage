use gpui::Window;

/// Clear Windows' maximized/minimized placement before GPUI changes content size.
/// SetWindowPos alone retains that placement and restores the wrong size later.
pub fn restore_before_resize(window: &Window) -> bool {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{IsIconic, IsZoomed, SW_RESTORE, ShowWindowAsync},
        };
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(handle) = handle.as_raw()
        {
            let hwnd = HWND(handle.hwnd.get() as *mut std::ffi::c_void);
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
