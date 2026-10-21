//! The update's two requests, both through the network gate: the feed (`Purpose::UpdateCheck`)
//! and, when the user presses Update Now, the disk image (`Purpose::UpdateDownload`). Only
//! `https://` addresses are fetched.
//!
//! Builds with the `update-test-hook` feature (never a release: `bundle-macos.sh` refuses it for
//! a version without `-test`) also read `file://` addresses, so the end-to-end dry run can serve a
//! signed feed and disk image from a folder (docs/lm/update-feed.md › Dry run).

use std::path::Path;
use std::time::Duration;

/// The largest feed read.
pub const MAX_FEED_BYTES: u64 = 64 * 1024;
/// The largest disk image downloaded.
pub const MAX_DMG_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// An address the update may use: `https://` with a host and nothing that could break out of a
/// link (and `file://` in test-hook builds).
pub fn usable(u: &str) -> bool {
    if u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    #[cfg(feature = "update-test-hook")]
    if u.strip_prefix("file:///").is_some_and(|p| !p.is_empty()) {
        return true;
    }
    u.strip_prefix("https://").is_some_and(|rest| !rest.split(['/', '?', '#']).next().unwrap_or("").is_empty())
}

#[cfg(feature = "update-test-hook")]
fn file_path(url: &str) -> Option<&Path> {
    url.strip_prefix("file://").map(Path::new)
}

/// Reads the feed at `url` (blocking: on a background thread).
#[cfg(not(target_arch = "wasm32"))]
pub fn feed(url: &str) -> Result<String, String> {
    #[cfg(feature = "update-test-hook")]
    if let Some(p) = file_path(url) {
        lm_net::check(lm_net::Purpose::UpdateCheck).map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        return if text.len() as u64 > MAX_FEED_BYTES { Err("feed too large".into()) } else { Ok(text) };
    }
    lm_net::get_text(lm_net::Purpose::UpdateCheck, url, Duration::from_secs(10), MAX_FEED_BYTES)
}

/// Downloads `url` to `dest` (blocking: on a background thread), first into `dest` + `.part`,
/// which is renamed when complete. `progress(bytes so far, size)` returns `false` to cancel.
#[cfg(not(target_arch = "wasm32"))]
pub fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(u64, Option<u64>) -> bool) -> Result<u64, String> {
    let part = dest.with_extension("part");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?);
    let r = get(url, &mut out, progress);
    drop(out);
    match r {
        Ok(n) => {
            std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            Ok(n)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn get(url: &str, out: &mut dyn std::io::Write, progress: &mut dyn FnMut(u64, Option<u64>) -> bool) -> Result<u64, String> {
    #[cfg(feature = "update-test-hook")]
    if let Some(p) = file_path(url) {
        use std::io::Read;
        lm_net::check(lm_net::Purpose::UpdateDownload).map_err(|e| e.to_string())?;
        let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let total = f.metadata().ok().map(|m| m.len());
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0u64;
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                return Ok(got);
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            got += n as u64;
            if !progress(got, total) {
                return Err(lm_net::CANCELLED.into());
            }
            // Slow enough that the progress bar can be seen in the dry run.
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    lm_net::download(lm_net::Purpose::UpdateDownload, url, MAX_DMG_BYTES, out, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_addresses_with_a_host() {
        assert!(usable("https://nimblemap.com/update/feed.json"));
        assert!(!usable("https://"));
        assert!(!usable("https:///x"));
        assert!(!usable("http://nimblemap.com/update/feed.json"));
        assert!(!usable("https://host/ with space"));
        assert!(!usable("javascript:alert(1)"));
        assert!(!usable(""));
        assert!(!usable("none"));
        assert_eq!(usable("file:///tmp/feed.json"), cfg!(feature = "update-test-hook"));
    }

    #[test]
    fn a_refused_download_leaves_no_file() {
        let dir = std::env::temp_dir().join(format!("lm-update-fetch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("x.dmg");
        assert!(download("http://example.invalid/x.dmg", &dest, &mut |_, _| true).is_err());
        assert!(!dest.exists() && !dest.with_extension("part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
