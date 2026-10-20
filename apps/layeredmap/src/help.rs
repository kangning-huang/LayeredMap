//! Help › NimbleMap Help (WP6.10): opens the help pages that ship inside the app in the default
//! browser, in the interface language (WP5.5) when the site has it. The pages are plain local HTML (`web/lm` in the repository, copied to
//! `Contents/Resources/help` by `packaging/lm/bundle-macos.sh`, and to `Resources/help` beside the
//! executable by `packaging/lm/linux/build-tarball.sh`). They load nothing from the
//! network, so help works offline and makes no request the network gate would have to allow.

use std::path::{Path, PathBuf};

use lm_core::i18n::Lang;

/// The pages in a development checkout.
const DEV_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/lm");

/// The help folder inside an app bundle, `<NimbleMap.app>/Contents/Resources/help`, for an
/// executable at `<NimbleMap.app>/Contents/MacOS/layeredmap` (on Linux `Resources/help` beside
/// `layeredmap`; `lm_core::platform::resources_dir`).
fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    Some(lm_core::platform::resources_dir(exe, lm_core::platform::Os::CURRENT)?.join("help"))
}

/// The site folder holding a language's pages (`web/lm/<folder>/`); English is at the root.
fn folder(lang: Lang) -> Option<&'static str> {
    match lang.code() {
        "en" => None,
        "pt" => Some("pt-br"),
        code => Some(code),
    }
}

/// The help start page in the interface language: the bundle's when it has one, otherwise the
/// checkout's; the English page when that language has none.
fn page(exe: Option<&Path>, lang: Lang, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let english = Path::new("help.html");
    let rels: Vec<PathBuf> = match folder(lang) {
        Some(f) => vec![Path::new(f).join("help.html"), english.to_path_buf()],
        None => vec![english.to_path_buf()],
    };
    let bundle = exe.and_then(bundle_dir);
    rels.iter()
        .flat_map(|rel| [bundle.as_ref().map(|d| d.join(rel)), Some(Path::new(DEV_DIR).join(rel))])
        .flatten()
        .find(|p| exists(p))
        .unwrap_or_else(|| Path::new(DEV_DIR).join(english))
}

/// Opens the help start page, in the interface language `lang`, in the default browser.
pub fn open(lang: Lang) {
    let exe = std::env::current_exe().ok();
    let path = page(exe.as_deref(), lang, |p| p.is_file());
    if !path.is_file() {
        log::warn!("help: {} is missing", path.display());
        return;
    }
    #[cfg(target_os = "macos")]
    if let Err(e) = std::process::Command::new("/usr/bin/open").arg(&path).spawn() {
        log::warn!("help: couldn't open {}: {e}", path.display());
    }
    // Linux and the other Unix desktops: the desktop's default browser.
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
        log::warn!("help: couldn't open {}: {e}", path.display());
    }
    #[cfg(not(unix))]
    log::info!("help: {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang(code: &str) -> Lang {
        Lang::from_code(code).unwrap_or(Lang::EN)
    }

    #[test]
    fn the_bundle_page_wins_when_present() {
        let (exe, want) = if cfg!(target_os = "macos") {
            (
                Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap"),
                PathBuf::from("/Applications/NimbleMap.app/Contents/Resources/help/zh/help.html"),
            )
        } else {
            (Path::new("/opt/NimbleMap/layeredmap"), PathBuf::from("/opt/NimbleMap/Resources/help/zh/help.html"))
        };
        assert_eq!(page(Some(exe), lang("zh"), |p| p == want), want);
        let dev = page(Some(exe), Lang::EN, |_| false);
        assert!(dev.ends_with("web/lm/help.html"), "{}", dev.display());
    }

    #[test]
    fn portuguese_lives_in_pt_br() {
        let p = page(None, lang("pt"), |p| p.is_file());
        assert!(p.ends_with("web/lm/pt-br/help.html"), "{}", p.display());
    }

    #[test]
    fn a_missing_page_falls_back_to_english() {
        let exe = Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap");
        let english = PathBuf::from("/Applications/NimbleMap.app/Contents/Resources/help/help.html");
        assert_eq!(page(Some(exe), lang("ja"), |p| p == english), english);
    }

    #[test]
    fn the_checkout_has_a_start_page_for_every_language() {
        for l in Lang::all() {
            let p = page(None, l, |p| p.is_file());
            assert!(p.is_file(), "{}", p.display());
            if let Some(f) = folder(l) {
                assert!(p.ends_with(Path::new(f).join("help.html")), "{}: {}", l.code(), p.display());
            }
        }
    }
}
