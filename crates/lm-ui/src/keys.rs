//! Keyboard shortcuts as the platform writes them (WP6.2): "⌘Z" and "⇧⌘Z" on macOS, "Ctrl+Z" and
//! "Ctrl+Shift+Z" on Windows. egui's `Modifiers::COMMAND` is already ⌘ on macOS and Ctrl
//! elsewhere; these are only the words shown to people.

/// The command key with `key`.
pub fn shortcut(key: &str) -> String {
    shortcut_for(cfg!(target_os = "macos"), key, false)
}

/// The command key and Shift with `key`.
pub fn shortcut_shift(key: &str) -> String {
    shortcut_for(cfg!(target_os = "macos"), key, true)
}

fn shortcut_for(mac: bool, key: &str, shift: bool) -> String {
    match (mac, shift) {
        (true, false) => format!("⌘{key}"),
        (true, true) => format!("⇧⌘{key}"),
        // Windows key names are localized on some keyboards ("Strg" in German).
        (false, false) => lm_core::i18n::fmt("Ctrl+{key}", &[("key", key)]),
        (false, true) => lm_core::i18n::fmt("Ctrl+Shift+{key}", &[("key", key)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_uses_symbols_and_windows_spells_ctrl() {
        assert_eq!(shortcut_for(true, "Z", false), "⌘Z");
        assert_eq!(shortcut_for(true, "Z", true), "⇧⌘Z");
        assert_eq!(shortcut_for(false, "0", false), "Ctrl+0");
        assert_eq!(shortcut_for(false, "Z", true), "Ctrl+Shift+Z");
        assert_eq!(shortcut("O"), if cfg!(target_os = "macos") { "⌘O" } else { "Ctrl+O" });
    }
}
