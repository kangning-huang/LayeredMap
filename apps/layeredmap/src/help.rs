//! Help › LayeredMap Help (WP6.10): opens the help pages that ship inside the app in the default
//! browser. The pages are plain local HTML (`web/lm` in the repository, copied to
//! `Contents/Resources/help` by `packaging/lm/bundle-macos.sh`). They load nothing from the
//! network, so help works offline and makes no request the network gate would have to allow.

use std::path::{Path, PathBuf};

/// The pages in a development checkout.
const DEV_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/lm");

/// The help folder inside an app bundle, `<LayeredMap.app>/Contents/Resources/help`, for an
/// executable at `<LayeredMap.app>/Contents/MacOS/layeredmap`.
fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    Some(exe.parent()?.parent()?.join("Resources").join("help"))
}

/// The help start page: the bundle's when it has one, otherwise the checkout's; in Simplified
/// Chinese when the system's first language is Chinese.
fn page(exe: Option<&Path>, chinese: bool, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let rel = if chinese { "zh/help.html" } else { "help.html" };
    exe.and_then(bundle_dir).map(|d| d.join(rel)).filter(|p| exists(p)).unwrap_or_else(|| Path::new(DEV_DIR).join(rel))
}

/// Opens the help start page in the default browser.
pub fn open() {
    let exe = std::env::current_exe().ok();
    let path = page(exe.as_deref(), lm_ui::update::prefers_chinese(), |p| p.is_file());
    if !path.is_file() {
        log::warn!("help: {} is missing", path.display());
        return;
    }
    #[cfg(target_os = "macos")]
    if let Err(e) = std::process::Command::new("/usr/bin/open").arg(&path).spawn() {
        log::warn!("help: couldn't open {}: {e}", path.display());
    }
    #[cfg(not(target_os = "macos"))]
    log::info!("help: {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_page_wins_when_present() {
        let exe = Path::new("/Applications/LayeredMap.app/Contents/MacOS/layeredmap");
        let want = PathBuf::from("/Applications/LayeredMap.app/Contents/Resources/help/zh/help.html");
        assert_eq!(page(Some(exe), true, |p| p == want), want);
        let dev = page(Some(exe), false, |_| false);
        assert!(dev.ends_with("web/lm/help.html"), "{}", dev.display());
    }

    #[test]
    fn the_checkout_has_both_start_pages() {
        for chinese in [false, true] {
            let p = page(None, chinese, |_| false);
            assert!(p.is_file(), "{}", p.display());
        }
    }
}
