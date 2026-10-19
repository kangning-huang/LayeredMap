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

/// The Pro tier's name.
pub const PRO_NAME: &str = "NimbleMap Pro";

/// The Chinese brand name (owner's choice, 2026-10-19): a pun on 凌波微步, light and nimble like the
/// English name, which the Chinese web pages show beside it ("凌波微图 NimbleMap"). Not shown in the
/// app yet: the interface is English-only until WP5.5.
pub const NAME_ZH: &str = "凌波微图";

/// [`NAME`] or [`PRO_NAME`].
pub fn name(pro: bool) -> &'static str {
    if pro { PRO_NAME } else { NAME }
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
        assert_eq!(name(false), NAME);
        assert_eq!(name(true), PRO_NAME);
        assert_eq!(with_version("1.2.3"), format!("{NAME} 1.2.3"));
    }
}
