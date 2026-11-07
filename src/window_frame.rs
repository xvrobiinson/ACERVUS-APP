//! The window's own frame: Windows' title bar stays (dragging, snapping and
//! its buttons work as everywhere), but without the app's name and icon in
//! it, so the bar is a clean strip above the hero. The taskbar and Alt+Tab
//! still show both.

/// Asks Windows not to draw the title text or the icon in the window's bar.
/// Does nothing where that isn't possible.
pub fn hide_title_and_icon(window: &gpui_kit::Window) {
    #[cfg(windows)]
    if let Some(h) = crate::file_picker::owner_of(window) {
        imp::hide(h);
    }
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Controls::{SetWindowThemeAttribute, WTA_NONCLIENT, WTA_OPTIONS, WTNCA_NODRAWCAPTION, WTNCA_NODRAWICON};
    use windows::Win32::UI::WindowsAndMessaging::{CreateIcon, ICON_SMALL, SendMessageW, WM_SETICON};

    pub fn hide(hwnd: isize) {
        let flags = WTNCA_NODRAWCAPTION | WTNCA_NODRAWICON;
        let options = WTA_OPTIONS { dwFlags: flags, dwMask: flags };
        // SAFETY: a live window of this process and a correctly sized struct.
        let done = unsafe {
            SetWindowThemeAttribute(
                HWND(hwnd as *mut _),
                WTA_NONCLIENT,
                &options as *const WTA_OPTIONS as *const _,
                std::mem::size_of::<WTA_OPTIONS>() as u32,
            )
        };
        if let Err(e) = done {
            log::warn!("could not hide the title bar's name and icon: {e}");
        }
        // The bar draws the window's small icon: a blank one. The taskbar and
        // Alt+Tab use the big icon (the program's own), which stays.
        let and = [0xFFu8; 32]; // 16 x 16, every pixel see-through
        let xor = [0u8; 32];
        // SAFETY: two 16 x 16 one-bit masks, as CreateIcon expects.
        match unsafe { CreateIcon(None, 16, 16, 1, 1, and.as_ptr(), xor.as_ptr()) } {
            Ok(icon) => unsafe {
                SendMessageW(HWND(hwnd as *mut _), WM_SETICON, Some(WPARAM(ICON_SMALL as usize)), Some(LPARAM(icon.0 as isize)));
            },
            Err(e) => log::warn!("could not blank the title bar's icon: {e}"),
        }
    }
}
