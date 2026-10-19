//! Native Windows hooks (WP6.2), under the same rule as the macOS module: the one place `unsafe`
//! is allowed, with a SAFETY comment on each block and a second review.
//!
//! The menu bar: `muda` draws a native Win32 menu bar on the app's window, given its HWND. Its
//! accelerators are shown in the menu ("Ctrl+O") but not translated by the message loop: the
//! shell already takes those keys from egui (`lm_ui::Shell::shortcuts`), and a text field being
//! edited must still get Ctrl+Z, Ctrl+C and Ctrl+V first, as on macOS.
//!
//! Open With needs no hook on Windows: Explorer starts `layeredmap.exe "<file>"`, and the command
//! line is opened like any other (main.rs). Drops on the window arrive through winit as egui's
//! dropped files.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// The window's HWND, from eframe's creation context.
pub fn hwnd(cc: &eframe::CreationContext<'_>) -> Option<isize> {
    match cc.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// Puts `menu` on the window `hwnd`; its bar follows the system's light or dark setting.
pub fn attach_menu(menu: &muda::Menu, hwnd: isize) -> bool {
    // SAFETY: `hwnd` comes from eframe's creation context: the app's one window, valid while the
    // event loop runs, which is while muda handles its menu messages. This runs on the main
    // thread, which created the window, as Win32 menus require. If the window is destroyed before
    // the menu is dropped, muda's clean-up calls fail on the stale handle without effect.
    match unsafe { menu.init_for_hwnd_with_theme(hwnd, muda::MenuTheme::Auto) } {
        Ok(()) => true,
        Err(e) => {
            log::error!("menu bar: {e}");
            false
        }
    }
}
