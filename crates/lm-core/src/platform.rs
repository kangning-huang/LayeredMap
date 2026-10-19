//! Where NimbleMap keeps things on each platform (WP6.2): the bundled resources next to the
//! executable, and the per-user folders. Pure path rules: callers pass the executable's path and
//! an environment lookup, so the macOS and Windows rules are both tested on either system.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The system the rules are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
    /// Linux and the rest (best effort, unpackaged).
    Other,
}

impl Os {
    /// The system this build runs on.
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Other
    };
}

/// The bundled resources folder (examples, reference layers, the tier file) for an executable.
/// macOS: `<App>.app/Contents/Resources` for `Contents/MacOS/layeredmap`. Windows: `Resources`
/// beside `layeredmap.exe`, as the installer lays it out (`packaging/lm/windows`).
pub fn resources_dir(exe: &Path, os: Os) -> Option<PathBuf> {
    let dir = exe.parent()?;
    match os {
        Os::Windows => Some(dir.join("Resources")),
        Os::MacOs | Os::Other => Some(dir.parent()?.join("Resources")),
    }
}

/// [`resources_dir`] for the running executable.
pub fn current_resources_dir() -> Option<PathBuf> {
    resources_dir(&std::env::current_exe().ok()?, Os::CURRENT)
}

/// A per-user folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folder {
    /// Small files that follow the user (the question log).
    Support,
    /// Autosaves and the recovery journal: can be large, so never in a roaming profile.
    Autosave,
    /// The log file and crash logs.
    Logs,
    /// Downloads that can be fetched again (the data library).
    Caches,
}

/// Where `folder` is for the user `env` describes. macOS: `~/Library/Application Support`,
/// `~/Library/Logs` and `~/Library/Caches`. Windows: the roaming `%APPDATA%` for Support and the
/// local `%LOCALAPPDATA%` for the rest (each falling back to `%USERPROFILE%\AppData\…`).
/// `None` when the environment names no home.
///
/// The folders keep the name `LayeredMap` after the rename to NimbleMap (`crate::brand`): they
/// hold the user's autosaves, question log and caches, so moving them needs a migration of its
/// own (docs/lm/naming.md).
pub fn user_dir(folder: Folder, os: Os, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let var = |k: &str| env(k).filter(|v| !v.is_empty()).map(PathBuf::from).filter(|p| p.is_absolute());
    match os {
        Os::Windows => {
            let profile = var("USERPROFILE");
            let roaming = var("APPDATA").or_else(|| profile.as_ref().map(|p| p.join("AppData").join("Roaming")));
            let local = var("LOCALAPPDATA").or_else(|| profile.as_ref().map(|p| p.join("AppData").join("Local")));
            Some(match folder {
                Folder::Support => roaming?.join("LayeredMap"),
                Folder::Autosave => local?.join("LayeredMap").join("Autosave"),
                Folder::Logs => local?.join("LayeredMap").join("Logs"),
                Folder::Caches => local?.join("LayeredMap").join("Cache"),
            })
        }
        Os::MacOs => {
            let lib = var("HOME")?.join("Library");
            Some(match folder {
                Folder::Support => lib.join("Application Support/LayeredMap"),
                Folder::Autosave => lib.join("Application Support/LayeredMap/Autosave"),
                Folder::Logs => lib.join("Logs/LayeredMap"),
                Folder::Caches => lib.join("Caches/LayeredMap"),
            })
        }
        // Unchanged from before Windows (best effort): the macOS-style support folder under the
        // home folder, `~/LayeredMap/logs`, and the XDG cache.
        Os::Other => {
            let home = var("HOME");
            Some(match folder {
                Folder::Support => home?.join("Library/Application Support/LayeredMap"),
                Folder::Autosave => home?.join("Library/Application Support/LayeredMap/Autosave"),
                Folder::Logs => home?.join("LayeredMap/logs"),
                Folder::Caches => match var("XDG_CACHE_HOME") {
                    Some(x) => x.join("layeredmap"),
                    None => home?.join(".cache/layeredmap"),
                },
            })
        }
    }
}

/// [`user_dir`] for this user on this system.
pub fn current_user_dir(folder: Folder) -> Option<PathBuf> {
    user_dir(folder, Os::CURRENT, |k| std::env::var_os(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
    }

    #[test]
    fn resources_sit_in_the_bundle_on_macos_and_beside_the_exe_on_windows() {
        let mac = Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap");
        assert_eq!(resources_dir(mac, Os::MacOs), Some(PathBuf::from("/Applications/NimbleMap.app/Contents/Resources")));
        // Built with `/` so the test reads the same on both systems; the rule is "the exe's folder".
        let win = Path::new("C:/Program Files/NimbleMap/layeredmap.exe");
        assert_eq!(resources_dir(win, Os::Windows), Some(PathBuf::from("C:/Program Files/NimbleMap/Resources")));
    }

    #[test]
    fn macos_folders_are_under_library() {
        let e = env(&[("HOME", "/Users/me")]);
        assert_eq!(user_dir(Folder::Support, Os::MacOs, &e), Some(PathBuf::from("/Users/me/Library/Application Support/LayeredMap")));
        assert_eq!(
            user_dir(Folder::Autosave, Os::MacOs, &e),
            Some(PathBuf::from("/Users/me/Library/Application Support/LayeredMap/Autosave"))
        );
        assert_eq!(user_dir(Folder::Logs, Os::MacOs, &e), Some(PathBuf::from("/Users/me/Library/Logs/LayeredMap")));
        assert_eq!(user_dir(Folder::Caches, Os::MacOs, &e), Some(PathBuf::from("/Users/me/Library/Caches/LayeredMap")));
        assert_eq!(user_dir(Folder::Logs, Os::MacOs, env(&[])), None);
    }

    /// Windows paths are only absolute on Windows, so the Windows rules are checked there and
    /// here only for the fallbacks that don't depend on it.
    #[test]
    fn windows_folders_are_roaming_for_support_and_local_for_the_rest() {
        let (roaming, local, profile) = if cfg!(windows) {
            (r"C:\Users\me\AppData\Roaming", r"C:\Users\me\AppData\Local", r"C:\Users\me")
        } else {
            ("/c/Users/me/AppData/Roaming", "/c/Users/me/AppData/Local", "/c/Users/me")
        };
        let vars = [("APPDATA", roaming), ("LOCALAPPDATA", local), ("HOME", "/ignored")];
        let e = env(&vars);
        assert_eq!(user_dir(Folder::Support, Os::Windows, &e), Some(Path::new(roaming).join("LayeredMap")));
        assert_eq!(user_dir(Folder::Autosave, Os::Windows, &e), Some(Path::new(local).join("LayeredMap").join("Autosave")));
        assert_eq!(user_dir(Folder::Logs, Os::Windows, &e), Some(Path::new(local).join("LayeredMap").join("Logs")));
        assert_eq!(user_dir(Folder::Caches, Os::Windows, &e), Some(Path::new(local).join("LayeredMap").join("Cache")));
        // Without the AppData variables, the profile's AppData folders.
        let vars = [("USERPROFILE", profile)];
        let e = env(&vars);
        assert_eq!(
            user_dir(Folder::Logs, Os::Windows, &e),
            Some(Path::new(profile).join("AppData").join("Local").join("LayeredMap").join("Logs"))
        );
        // A relative or empty value is never used (it would write into the working folder).
        assert_eq!(user_dir(Folder::Support, Os::Windows, env(&[("APPDATA", "AppData"), ("USERPROFILE", "")])), None);
    }
}
