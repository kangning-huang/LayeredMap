//! The app's display name, in one place (docs/lm/naming.md).
//!
//! "NimbleMap" is a working name: the trademark check is still pending, so a rename must be one
//! edit here plus the few literals that can't read a `const`. Those carry a comment naming
//! `lm_core::brand`; `git grep -n "lm_core::brand"` finds them all, and so does the list in
//! docs/lm/naming.md (shell scripts, the Info.plist, the installer and `build.rs` each take the
//! name from one variable at the top of the file).
//!
//! Only the name people see lives here. Internal identifiers stay as they are: the crate names,
//! the binary `layeredmap`, bundle identifiers (`app.layeredmap…`), the `.lmap` extension, the
//! keychain service, environment variables and the user folders in [`crate::platform`].

/// The app's name as people see it: menus, window titles, messages, file metadata.
pub const NAME: &str = "NimbleMap";

/// The name of what signing in unlocks, in messages ("Sign in to use NimbleMap Pro"). There is one
/// app, always called [`NAME`]; the toolbar marks unlocked Pro with a small badge.
pub const PRO_NAME: &str = "NimbleMap Pro";

/// The Chinese brand name (owner's choice, 2026-10-19): a pun on 凌波微步, light and nimble like the
/// English name, which the Chinese web pages show beside it ("凌波微图 NimbleMap"). The Chinese
/// interface shows it in place of [`NAME`] (owner's choice, 2026-10-19; see [`shown`]). Japanese
/// and Korean use [`NAME`]: no kanji or hangul form (docs/lm/localization.md).
pub const NAME_ZH: &str = "凌波微图";

/// [`PRO_NAME`] in Chinese.
pub const PRO_NAME_ZH: &str = "凌波微图 Pro";

/// The name the interface shows in its current language (`lm_core::i18n`): [`NAME_ZH`] in
/// Chinese, [`NAME`] in every other language; with `pro`, the name of the Pro features. Only for
/// text people read in the interface: files, logs and reports keep [`NAME`].
pub fn shown(pro: bool) -> &'static str {
    shown_in(crate::i18n::current(), pro)
}

/// [`shown`] for a given interface language.
pub fn shown_in(lang: crate::i18n::Lang, pro: bool) -> &'static str {
    match (lang.code(), pro) {
        ("zh", false) => NAME_ZH,
        ("zh", true) => PRO_NAME_ZH,
        (_, false) => NAME,
        (_, true) => PRO_NAME,
    }
}

/// The name with a version, as recorded in a file's or a recipe's `software` field:
/// "NimbleMap 0.5.0".
pub fn with_version(version: &str) -> String {
    format!("{NAME} {version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_agree() {
        assert!(PRO_NAME.starts_with(NAME) && PRO_NAME.ends_with(" Pro"));
        assert_eq!(with_version("1.2.3"), format!("{NAME} 1.2.3"));
        assert!(PRO_NAME_ZH.starts_with(NAME_ZH) && PRO_NAME_ZH.ends_with(" Pro"));
    }

    #[test]
    fn chinese_interface_shows_the_chinese_name() {
        use crate::i18n::Lang;
        let zh = Lang::from_code("zh").unwrap();
        assert_eq!(shown_in(zh, false), NAME_ZH);
        assert_eq!(shown_in(zh, true), PRO_NAME_ZH);
        for code in ["en", "ja", "ko", "de"] {
            let l = Lang::from_code(code).unwrap();
            assert_eq!(shown_in(l, false), NAME, "{code}");
            assert_eq!(shown_in(l, true), PRO_NAME, "{code}");
        }
        // Tests run in English.
        assert_eq!(shown(false), NAME);
    }
}
