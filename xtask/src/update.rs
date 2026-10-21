//! `cargo xtask update-keygen` and `cargo xtask update-sign <dmg> <version>`: the update signing
//! key and the signed feed (docs/lm/update-feed.md › Signing).
//!
//! The private key lives only in the macOS keychain (generic password, service
//! `nimblemap-update`, account `ed25519`, base64 PKCS#8). It is written there through `security -i`
//! on standard input, so it never appears in a process list, and read back with
//! `security find-generic-password -w` into memory. Neither command prints it or writes it to a
//! file. The public key is compiled into the app (`crates/lm-ui/src/update/verify.rs`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine as _;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};

/// The keychain item holding the private key.
pub const SERVICE: &str = "nimblemap-update";
pub const ACCOUNT: &str = "ed25519";

/// Where the app's public key is written (the `PUBLIC_KEY` constant).
const VERIFY_RS: &str = "crates/lm-ui/src/update/verify.rs";
/// The placeholder `update-keygen` replaces.
const PLACEHOLDER: &str = "__UPDATE_PUBLIC_KEY__";

/// The release asset's name and where releases are downloaded from.
const DMG_NAME: &str = "NimbleMap-macos-arm64.dmg";
const GITHUB: &str = "https://github.com/kangning-huang/NimbleMap/releases/download";
const GITCODE: &str = "https://gitcode.com/KN-Huang/NimbleMap/releases/download";
const PAGE: &str = "https://nimblemap.com/download.html";

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// The bytes signed: must stay identical to `lm_ui::update::verify::signed_message` (a test
/// checks the app's source).
pub fn signed_message(version: &str, sha256: &str) -> String {
    format!("NimbleMap update\nformat 1\nversion {version}\nsha256 {sha256}\n")
}

/// Whether the keychain already holds a signing key.
fn key_exists() -> bool {
    Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The private key from the keychain (never printed).
fn read_key() -> Result<Ed25519KeyPair, String> {
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("security: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "no update signing key in the keychain (service {SERVICE}, account {ACCOUNT}); restore it from the backup, or run cargo xtask update-keygen for a first key"
        ));
    }
    let der = b64().decode(String::from_utf8_lossy(&out.stdout).trim()).map_err(|_| "the keychain item is not a base64 key".to_string())?;
    Ed25519KeyPair::from_pkcs8(&der).map_err(|_| "the keychain item is not an Ed25519 key".to_string())
}

/// The public key compiled into the app, from its source.
fn app_public_key(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join(VERIFY_RS)).map_err(|e| format!("{VERIFY_RS}: {e}"))?;
    let line = text.lines().find(|l| l.starts_with("pub const PUBLIC_KEY: &str = \"")).ok_or(format!("{VERIFY_RS}: no PUBLIC_KEY"))?;
    Ok(line.split('"').nth(1).unwrap_or_default().to_string())
}

/// `cargo xtask update-keygen`: makes the signing key pair, stores the private key in the
/// keychain and writes the public key into the app's source. Refuses when a key exists: a new key
/// would make every installed app reject future updates.
pub fn keygen(root: &Path) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("update-keygen keeps the key in the macOS keychain: run it on the owner's Mac".into());
    }
    if key_exists() {
        return Err(format!(
            "the keychain already holds an update signing key (service {SERVICE}, account {ACCOUNT}); refusing to replace it. Installed apps trust only that key."
        ));
    }
    let current = app_public_key(root)?;
    if current != PLACEHOLDER {
        return Err(format!(
            "{VERIFY_RS} already has a public key ({current}) but the keychain has no private key: restore the private key from its backup instead of making a new pair"
        ));
    }
    let rng = ring::rand::SystemRandom::new();
    let doc = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| "couldn't generate a key")?;
    let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).map_err(|_| "couldn't read the new key")?;
    let public = b64().encode(pair.public_key().as_ref());
    let secret = b64().encode(doc.as_ref());
    // `security -i` reads the command from standard input: the key is never an argument.
    let mut child = Command::new("/usr/bin/security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("security: {e}"))?;
    {
        let stdin = child.stdin.as_mut().ok_or("security: no stdin")?;
        writeln!(
            stdin,
            "add-generic-password -s {SERVICE} -a {ACCOUNT} -l NimbleMap-update-signing-key -j Ed25519-PKCS8-base64.Back-me-up:losing-me-means-installed-apps-cant-verify-updates -w {secret}"
        )
        .map_err(|e| format!("security: {e}"))?;
        writeln!(stdin, "quit").map_err(|e| format!("security: {e}"))?;
    }
    drop(secret);
    child.wait().map_err(|e| format!("security: {e}"))?;
    // Read it back: the key in the keychain must be the one just made.
    let stored = read_key()?;
    if stored.public_key().as_ref() != pair.public_key().as_ref() {
        return Err("the key read back from the keychain differs from the one made".into());
    }
    let path = root.join(VERIFY_RS);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{VERIFY_RS}: {e}"))?;
    std::fs::write(&path, text.replacen(PLACEHOLDER, &public, 1)).map_err(|e| format!("{VERIFY_RS}: {e}"))?;
    println!("update-keygen: the private key is in the keychain (service {SERVICE}, account {ACCOUNT}); back it up now:");
    println!("  docs/lm/update-feed.md › The signing key");
    println!("update-keygen: public key {public}, written to {VERIFY_RS}");
    Ok(())
}

/// A file's SHA-256, lowercase hex.
fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    Ok(ctx.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect())
}

/// `MAJOR.MINOR.PATCH` with an optional `-pre` suffix of letters, digits, dots and hyphens.
fn valid_version(v: &str) -> bool {
    let (num, pre) = v.split_once('-').unwrap_or((v, ""));
    let parts: Vec<&str> = num.split('.').collect();
    (1..=4).contains(&parts.len())
        && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && pre.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !(v.contains('-') && pre.is_empty())
}

/// The app version inside a disk image (mounted read-only and hidden, then detached).
fn dmg_version(dmg: &Path) -> Result<String, String> {
    let mount = std::env::temp_dir().join(format!("xtask-update-sign-{}", std::process::id()));
    std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
    let st = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-quiet", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mount)
        .arg(dmg)
        .status()
        .map_err(|e| format!("hdiutil: {e}"))?;
    if !st.success() {
        return Err(format!("{}: hdiutil couldn't mount it", dmg.display()));
    }
    let app = std::fs::read_dir(&mount)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app") && !p.is_symlink());
    let v = app.ok_or("the disk image holds no app".to_string()).and_then(|app| {
        let out = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
            .arg(app.join("Contents/Info.plist"))
            .output()
            .map_err(|e| format!("plutil: {e}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    });
    let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    let _ = std::fs::remove_dir(&mount);
    v
}

/// The feed for a signed release (docs/lm/update-feed.md › Format).
pub struct Release<'a> {
    pub version: &'a str,
    pub sha256: &'a str,
    pub signature: &'a str,
    pub github: String,
    pub gitcode: String,
    pub released: &'a str,
    pub notes: Option<serde_json::Value>,
    pub download_windows: Option<serde_json::Value>,
}

pub fn feed_json(r: &Release) -> serde_json::Value {
    let mut feed = serde_json::json!({
        "format": 1,
        "version": r.version,
        "released": r.released,
        // "free" is what every app reads (GitHub); "pro" is for Pro apps from 0.7.0 and earlier;
        // "gitcode" is the mainland China mirror (0.7.2 and later).
        "download": { "free": r.github, "pro": r.github, "gitcode": r.gitcode },
        "sha256": { "free": r.sha256, "pro": r.sha256 },
        "signature": r.signature,
        "page": PAGE,
        "minimum_macos": "13.0",
    });
    if let Some(n) = &r.notes {
        feed["notes"] = n.clone();
    }
    if let Some(w) = &r.download_windows {
        feed["download_windows"] = w.clone();
    }
    feed
}

/// `cargo xtask update-sign <dmg> <version> [--out FILE] [--notes TEXT] [--notes-zh TEXT]
/// [--base-github URL] [--base-gitcode URL]`: signs the release with the keychain's key and
/// writes the feed (default `web/lm/update/feed.json`, which `tools/lm/site/stamp.py` publishes
/// as https://nimblemap.com/update/feed.json).
pub fn sign(root: &Path, args: &[String]) -> Result<(), String> {
    let usage = "usage: cargo xtask update-sign <dmg> <version> [--out FILE] [--notes TEXT] [--notes-zh TEXT] [--base-github URL] [--base-gitcode URL]";
    let mut pos = Vec::new();
    let mut opt = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            opt.insert(name.to_string(), it.next().ok_or(usage)?.clone());
        } else {
            pos.push(a.clone());
        }
    }
    let [dmg, version] = pos.as_slice() else { return Err(usage.into()) };
    let dmg = PathBuf::from(dmg);
    let version = version.trim_start_matches('v');
    if !valid_version(version) {
        return Err(format!("{version}: not a version (MAJOR.MINOR.PATCH[-pre])"));
    }
    let sha = sha256_file(&dmg)?;
    // The checksum file published beside the disk image must agree.
    let sidecar = PathBuf::from(format!("{}.sha256", dmg.display()));
    if let Ok(text) = std::fs::read_to_string(&sidecar)
        && text.split_whitespace().next() != Some(sha.as_str())
    {
        return Err(format!("{}: doesn't match the disk image ({sha})", sidecar.display()));
    }
    if cfg!(target_os = "macos") {
        let inside = dmg_version(&dmg)?;
        if inside != version {
            return Err(format!("{}: holds NimbleMap {inside}, not {version}", dmg.display()));
        }
    }
    let pair = read_key()?;
    let public = b64().encode(pair.public_key().as_ref());
    let compiled = app_public_key(root)?;
    if public != compiled {
        return Err(format!(
            "the keychain's key doesn't match the app's public key in {VERIFY_RS} ({compiled}); installed apps would reject this feed"
        ));
    }
    let signature = b64().encode(pair.sign(signed_message(version, &sha).as_bytes()).as_ref());
    // Check what was made, as the app will.
    let raw = b64().decode(&signature).map_err(|e| e.to_string())?;
    UnparsedPublicKey::new(&ED25519, pair.public_key().as_ref())
        .verify(signed_message(version, &sha).as_bytes(), &raw)
        .map_err(|_| "the new signature doesn't verify")?;

    let out = opt.get("out").map(PathBuf::from).unwrap_or_else(|| root.join("web/lm/update/feed.json"));
    // Notes and Windows downloads already written for this version are kept.
    let old: Option<serde_json::Value> = std::fs::read_to_string(&out)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|f: &serde_json::Value| f["version"] == version);
    let mut notes = old.as_ref().and_then(|f| f.get("notes").cloned());
    for (key, lang) in [("notes", "en"), ("notes-zh", "zh")] {
        if let Some(text) = opt.get(key) {
            let n = notes.get_or_insert_with(|| serde_json::json!({}));
            n[lang] = serde_json::Value::String(text.clone());
        }
    }
    let base = |key: &str, default: &str| {
        format!("{}/{DMG_NAME}", opt.get(key).map_or_else(|| format!("{default}/v{version}"), |b| b.trim_end_matches('/').to_string()))
    };
    let released = Command::new("/bin/date")
        .arg("+%Y-%m-%d")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let feed = feed_json(&Release {
        version,
        sha256: &sha,
        signature: &signature,
        github: base("base-github", GITHUB),
        gitcode: base("base-gitcode", GITCODE),
        released: &released,
        notes,
        download_windows: old.as_ref().and_then(|f| f.get("download_windows").cloned()),
    });
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&feed).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("update-sign: {} signed (sha256 {sha})", dmg.display());
    println!("update-sign: wrote {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signed_bytes_match_the_apps() {
        let app = include_str!("../../crates/lm-ui/src/update/verify.rs");
        assert!(app.contains(r#"format!("NimbleMap update\nformat 1\nversion {version}\nsha256 {sha256}\n")"#));
        assert_eq!(signed_message("0.7.2", "ab"), "NimbleMap update\nformat 1\nversion 0.7.2\nsha256 ab\n");
    }

    #[test]
    fn the_app_has_a_public_key() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let k = app_public_key(root).unwrap();
        assert_eq!(b64().decode(&k).map(|k| k.len()), Ok(32), "{k}");
    }

    #[test]
    fn versions() {
        for v in ["0.7.2", "1.0", "0.7.3-test", "1.0.0-beta.2"] {
            assert!(valid_version(v), "{v}");
        }
        for v in ["", "v0.7.2", "0.7.x", "0.7.2-", "0.7.2 ", "1.2.3.4.5", "0.7.2-a b"] {
            assert!(!valid_version(v), "{v}");
        }
    }

    #[test]
    fn the_feed_names_both_downloads_and_the_signature() {
        let f = feed_json(&Release {
            version: "0.7.2",
            sha256: "ab",
            signature: "c2ln",
            github: format!("{GITHUB}/v0.7.2/{DMG_NAME}"),
            gitcode: format!("{GITCODE}/v0.7.2/{DMG_NAME}"),
            released: "2026-10-22",
            notes: Some(serde_json::json!({"en": "In-app updates."})),
            download_windows: None,
        });
        assert_eq!(f["format"], 1);
        assert_eq!(f["download"]["free"], "https://github.com/kangning-huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg");
        assert_eq!(f["download"]["pro"], f["download"]["free"]);
        assert_eq!(f["download"]["gitcode"], "https://gitcode.com/KN-Huang/NimbleMap/releases/download/v0.7.2/NimbleMap-macos-arm64.dmg");
        assert_eq!(f["sha256"]["free"], "ab");
        assert_eq!(f["signature"], "c2ln");
        assert_eq!(f["notes"]["en"], "In-app updates.");
        assert!(f.get("download_windows").is_none());
    }
}
