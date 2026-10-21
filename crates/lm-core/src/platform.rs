//! Where NimbleMap keeps things on each platform (WP6.2): the bundled resources next to the
//! executable, and the per-user folders. Pure path rules: callers pass the executable's path and
//! an environment lookup, so the macOS, Windows and Linux rules are all tested on any system.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The system the rules are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
    /// Linux and the rest (best effort: a portable tarball, `packaging/lm/linux`).
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

/// The bundled resources folder (examples, help, reference layers, the account config) for an executable.
/// macOS: `<App>.app/Contents/Resources` for `Contents/MacOS/layeredmap`. Windows: `Resources`
/// beside `layeredmap.exe`, as the installer lays it out (`packaging/lm/windows`). Linux and the
/// rest: `Resources` beside `layeredmap` too, as the tarball lays it out (`packaging/lm/linux`).
pub fn resources_dir(exe: &Path, os: Os) -> Option<PathBuf> {
    let dir = exe.parent()?;
    match os {
        Os::Windows | Os::Other => Some(dir.join("Resources")),
        Os::MacOs => Some(dir.parent()?.join("Resources")),
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
/// Linux and the rest: the XDG base directories, `$XDG_DATA_HOME` (`~/.local/share`) for Support,
/// `$XDG_STATE_HOME` (`~/.local/state`) for autosaves and logs, `$XDG_CACHE_HOME` (`~/.cache`)
/// for caches, each in a `layeredmap` folder. `None` when the environment names no home.
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
        // The XDG base directories (best effort; docs/lm/linux.md). Lower-case `layeredmap`, as
        // Linux folders are named. Builds before the tarball used a macOS-style folder under the
        // home folder; nothing moves it, as Linux had no users then.
        Os::Other => {
            let xdg = |k: &str, fallback: &str| var(k).or_else(|| var("HOME").map(|h| h.join(fallback)));
            Some(match folder {
                Folder::Support => xdg("XDG_DATA_HOME", ".local/share")?.join("layeredmap"),
                Folder::Autosave => xdg("XDG_STATE_HOME", ".local/state")?.join("layeredmap/autosave"),
                Folder::Logs => xdg("XDG_STATE_HOME", ".local/state")?.join("layeredmap/logs"),
                Folder::Caches => xdg("XDG_CACHE_HOME", ".cache")?.join("layeredmap"),
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
    fn resources_sit_in_the_bundle_on_macos_and_beside_the_exe_on_windows_and_linux() {
        let mac = Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap");
        assert_eq!(resources_dir(mac, Os::MacOs), Some(PathBuf::from("/Applications/NimbleMap.app/Contents/Resources")));
        // Built with `/` so the test reads the same on both systems; the rule is "the exe's folder".
        let win = Path::new("C:/Program Files/NimbleMap/layeredmap.exe");
        assert_eq!(resources_dir(win, Os::Windows), Some(PathBuf::from("C:/Program Files/NimbleMap/Resources")));
        // The tarball's root, wherever it was unpacked.
        let linux = Path::new("/home/me/NimbleMap-0.7.0-12-linux-x86_64/layeredmap");
        assert_eq!(resources_dir(linux, Os::Other), Some(PathBuf::from("/home/me/NimbleMap-0.7.0-12-linux-x86_64/Resources")));
    }

    #[test]
    #[cfg_attr(windows, ignore = "Unix paths are not absolute on Windows")]
    fn linux_folders_follow_xdg() {
        // Only HOME: the XDG defaults under it.
        let e = env(&[("HOME", "/home/me")]);
        assert_eq!(user_dir(Folder::Support, Os::Other, &e), Some(PathBuf::from("/home/me/.local/share/layeredmap")));
        assert_eq!(user_dir(Folder::Autosave, Os::Other, &e), Some(PathBuf::from("/home/me/.local/state/layeredmap/autosave")));
        assert_eq!(user_dir(Folder::Logs, Os::Other, &e), Some(PathBuf::from("/home/me/.local/state/layeredmap/logs")));
        assert_eq!(user_dir(Folder::Caches, Os::Other, &e), Some(PathBuf::from("/home/me/.cache/layeredmap")));
        // The XDG variables win over HOME.
        let vars = [("HOME", "/home/me"), ("XDG_DATA_HOME", "/d"), ("XDG_STATE_HOME", "/s"), ("XDG_CACHE_HOME", "/c")];
        let e = env(&vars);
        assert_eq!(user_dir(Folder::Support, Os::Other, &e), Some(PathBuf::from("/d/layeredmap")));
        assert_eq!(user_dir(Folder::Autosave, Os::Other, &e), Some(PathBuf::from("/s/layeredmap/autosave")));
        assert_eq!(user_dir(Folder::Logs, Os::Other, &e), Some(PathBuf::from("/s/layeredmap/logs")));
        assert_eq!(user_dir(Folder::Caches, Os::Other, &e), Some(PathBuf::from("/c/layeredmap")));
        // Without HOME an XDG variable still names its folder, and the others have none.
        let e = env(&[("XDG_STATE_HOME", "/s")]);
        assert_eq!(user_dir(Folder::Logs, Os::Other, &e), Some(PathBuf::from("/s/layeredmap/logs")));
        assert_eq!(user_dir(Folder::Support, Os::Other, &e), None);
        // A relative or empty XDG value is ignored, as the spec says: the default is used instead.
        let e = env(&[("HOME", "/home/me"), ("XDG_DATA_HOME", "rel"), ("XDG_CACHE_HOME", "")]);
        assert_eq!(user_dir(Folder::Support, Os::Other, &e), Some(PathBuf::from("/home/me/.local/share/layeredmap")));
        assert_eq!(user_dir(Folder::Caches, Os::Other, &e), Some(PathBuf::from("/home/me/.cache/layeredmap")));
        assert_eq!(user_dir(Folder::Caches, Os::Other, env(&[])), None);
    }

    /// `/Users/me` is only absolute off Windows, where the rule refuses it as it should.
    #[test]
    #[cfg_attr(windows, ignore = "Unix paths are not absolute on Windows")]
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
