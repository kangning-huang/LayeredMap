//! Installing a verified update on macOS (docs/lm/update-feed.md › Installing).
//!
//! 1. [`placement`]: the app must run from a folder it can write to. Running from the disk image,
//!    from App Translocation (a quarantined app opened where it was downloaded), outside an app
//!    bundle, or from a folder the user can't write to (a standard user and /Applications), the
//!    app doesn't try: it opens the downloaded disk image in Finder instead, with a short note.
//! 2. [`stage`]: the disk image is mounted read-only and hidden (`hdiutil attach -nobrowse`), the
//!    app in it is copied with `ditto` into a hidden sibling of the running bundle
//!    (`.NimbleMap-update-<pid>`, no `.app`, so Launch Services ignores it), and the copy's
//!    Info.plist must name the expected version and the running app's bundle id, its code
//!    signature must verify (`codesign --verify --strict`) and be the owner's Developer ID
//!    ([`TEAM_ID`]; releases are signed and notarized from 0.7.2 on). That is a second check on top
//!    of the feed's Ed25519 signature. The image is detached. Nothing has changed yet.
//! 3. When the app quits ([`finish`], from the app's `on_exit`, after the user's state is saved):
//!    the running bundle is renamed aside (`.NimbleMap-old-<pid>`), the copy renamed into its
//!    place (put back if that fails), and the new app opened (NSWorkspace, new instance). The new app removes the
//!    old copy at launch ([`after_update`]).
//!
//! The disk image is downloaded by the app itself, which doesn't ask macOS to quarantine its
//! downloads (no `LSFileQuarantineEnabled`), so neither the image nor the installed app carries the
//! quarantine attribute and Gatekeeper doesn't stop the new version with "Open Anyway".

use std::path::{Path, PathBuf};
use std::process::Command;

/// The Team ID of the owner's Developer ID certificate ("Developer ID Application: Kangning Huang
/// (75MXK7UJ6W)"), which every release from 0.7.2 on is signed with.
pub const TEAM_ID: &str = "75MXK7UJ6W";

/// Whether a new bundle's signing team is acceptable: the owner's, or (tests and the update dry
/// run's test-hook builds only) an ad-hoc signature, which has no team.
pub fn team_allowed(team: Option<&str>) -> bool {
    match team {
        Some(t) => t == TEAM_ID,
        None => cfg!(test) || cfg!(feature = "update-test-hook"),
    }
}

/// The Team ID an app is signed with (`None`: ad-hoc, no team).
pub fn team_id(app: &Path) -> Result<Option<String>, String> {
    let out =
        Command::new("/usr/bin/codesign").args(["--display", "--verbose=2"]).arg(app).output().map_err(|e| format!("codesign: {e}"))?;
    if !out.status.success() {
        return Err(format!("codesign: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    // codesign writes the details to standard error.
    let text = String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    let team = text.lines().find_map(|l| l.strip_prefix("TeamIdentifier=")).map(str::trim).unwrap_or("not set");
    Ok((team != "not set" && !team.is_empty()).then(|| team.to_string()))
}

/// Why the app can't replace itself where it runs (it opens the disk image in Finder instead).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manual {
    /// Not running from an app bundle (a development build).
    NotABundle,
    /// Running from the read-only disk image itself.
    OnDiskImage,
    /// Running from App Translocation's temporary copy.
    Translocated,
    /// The folder holding the app can't be written (a standard user and /Applications).
    NotWritable,
}

/// A new version copied next to the running app, ready to be swapped in at quit.
#[derive(Clone, Debug, PartialEq)]
pub struct Staged {
    /// The running app bundle (`/Applications/NimbleMap.app`).
    pub bundle: PathBuf,
    /// The verified copy beside it (`/Applications/.NimbleMap-update-<pid>`).
    pub new_app: PathBuf,
    /// The downloaded disk image (opened in Finder if the swap fails).
    pub dmg: PathBuf,
    pub version: String,
}

/// The app bundle an executable runs from: `X.app` for `X.app/Contents/MacOS/<exe>`.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app").then(|| app.to_path_buf())
}

/// The running app's bundle.
pub fn running_bundle() -> Option<PathBuf> {
    bundle_of(&std::env::current_exe().ok()?)
}

/// The bundle's name without `.app` ("NimbleMap").
fn stem(bundle: &Path) -> String {
    bundle.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "NimbleMap".into())
}

/// The hidden sibling a new version is staged in, and the one the old version is moved to.
fn sibling(bundle: &Path, kind: &str, pid: u32) -> Option<PathBuf> {
    Some(bundle.parent()?.join(format!(".{}-{kind}-{pid}", stem(bundle))))
}

/// Whether `p` is an old version moved aside by [`swap`] (only those are ever deleted).
pub fn is_old_copy(p: &Path) -> bool {
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name.strip_prefix('.')
        .and_then(|n| n.rsplit_once("-old-"))
        .is_some_and(|(app, pid)| !app.is_empty() && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

/// What the path alone says about where the app runs.
pub fn path_problem(bundle: &Path) -> Option<Manual> {
    let s = bundle.to_string_lossy();
    if s.contains("/AppTranslocation/") {
        return Some(Manual::Translocated);
    }
    None
}

/// Whether the app can replace itself at `bundle` (`None`: not in a bundle).
pub fn placement(bundle: Option<&Path>) -> Result<(), Manual> {
    let bundle = bundle.ok_or(Manual::NotABundle)?;
    if let Some(m) = path_problem(bundle) {
        return Err(m);
    }
    let probe = sibling(bundle, "probe", std::process::id()).ok_or(Manual::NotWritable)?;
    match std::fs::create_dir(&probe) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&probe);
            Ok(())
        }
        // A read-only volume under /Volumes is the disk image the app was opened from.
        Err(_) if bundle.starts_with("/Volumes") => Err(Manual::OnDiskImage),
        Err(_) => Err(Manual::NotWritable),
    }
}

/// Runs a system tool; its error output on failure.
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    let out = Command::new(program).args(args).output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let name = Path::new(program).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Err(format!("{name}: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// A string value from an app's Info.plist.
pub fn plist_value(app: &Path, key: &str) -> Result<String, String> {
    let plist = app.join("Contents/Info.plist");
    run("/usr/bin/plutil", &["-extract".as_ref(), key.as_ref(), "raw".as_ref(), "-o".as_ref(), "-".as_ref(), plist.as_os_str()])
}

/// The app inside a mounted disk image: the one named like the running bundle, else the first.
fn find_app(mount: &Path, name: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    if let Some(n) = name {
        let p = mount.join(n);
        if p.join("Contents/Info.plist").is_file() {
            return Ok(p);
        }
    }
    let mut apps: Vec<PathBuf> = std::fs::read_dir(mount)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "app") && !p.is_symlink() && p.join("Contents/Info.plist").is_file())
        .collect();
    apps.sort();
    apps.into_iter().next().ok_or_else(|| "the disk image holds no app".into())
}

/// Mounts `dmg`, copies its app beside `bundle` and checks the copy (step 2 above). Nothing is
/// changed when it fails.
pub fn stage(dmg: &Path, bundle: &Path, version: &str) -> Result<Staged, String> {
    let pid = std::process::id();
    let new_app = sibling(bundle, "update", pid).ok_or("the app has no folder")?;
    if new_app.exists() {
        std::fs::remove_dir_all(&new_app).map_err(|e| format!("{}: {e}", new_app.display()))?;
    }
    let mount = std::env::temp_dir().join(format!("nimblemap-update-mount-{pid}"));
    std::fs::create_dir_all(&mount).map_err(|e| format!("{}: {e}", mount.display()))?;
    // Read-only and hidden from Finder; the checksum was checked already, so hdiutil's own
    // verification is skipped.
    let attached = run(
        "/usr/bin/hdiutil",
        &[
            "attach".as_ref(),
            "-nobrowse".as_ref(),
            "-readonly".as_ref(),
            "-noautoopen".as_ref(),
            "-noverify".as_ref(),
            "-mountpoint".as_ref(),
            mount.as_os_str(),
            dmg.as_os_str(),
        ],
    );
    let was_attached = attached.is_ok();
    let copied = attached.and_then(|_| {
        let src = find_app(&mount, bundle.file_name())?;
        run("/usr/bin/ditto", &[src.as_os_str(), new_app.as_os_str()])?;
        let v = plist_value(&new_app, "CFBundleShortVersionString")?;
        if v != version {
            return Err(format!("the disk image holds version {v}, not {version}"));
        }
        let id = plist_value(&new_app, "CFBundleIdentifier")?;
        let ours = plist_value(bundle, "CFBundleIdentifier")?;
        if id != ours {
            return Err(format!("the disk image holds {id}, not {ours}"));
        }
        run("/usr/bin/codesign", &["--verify".as_ref(), "--strict".as_ref(), new_app.as_os_str()])?;
        let team = team_id(&new_app)?;
        if !team_allowed(team.as_deref()) {
            return Err(format!(
                "the new app is signed by {}, not the developer ({TEAM_ID})",
                team.as_deref().unwrap_or("no one (ad-hoc)")
            ));
        }
        Ok(())
    });
    if was_attached {
        detach(&mount);
    }
    let _ = std::fs::remove_dir(&mount);
    match copied {
        Ok(()) => Ok(Staged { bundle: bundle.to_path_buf(), new_app, dmg: dmg.to_path_buf(), version: version.to_string() }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&new_app);
            Err(e)
        }
    }
}

fn detach(mount: &Path) {
    if run("/usr/bin/hdiutil", &["detach".as_ref(), mount.as_os_str()]).is_err()
        && let Err(e) = run("/usr/bin/hdiutil", &["detach".as_ref(), "-force".as_ref(), mount.as_os_str()])
    {
        log::warn!("update: couldn't detach {}: {e}", mount.display());
    }
}

/// Renames the running bundle aside and the staged copy into its place; puts the old one back if
/// the second rename fails. Returns where the old version went.
pub fn swap(s: &Staged) -> Result<PathBuf, String> {
    let old = sibling(&s.bundle, "old", std::process::id()).ok_or("the app has no folder")?;
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(|e| format!("{}: {e}", old.display()))?;
    }
    std::fs::rename(&s.bundle, &old).map_err(|e| format!("{}: {e}", s.bundle.display()))?;
    if let Err(e) = std::fs::rename(&s.new_app, &s.bundle) {
        if let Err(back) = std::fs::rename(&old, &s.bundle) {
            log::error!("update: couldn't put {} back: {back}", s.bundle.display());
        }
        return Err(format!("{}: {e}", s.bundle.display()));
    }
    Ok(old)
}

/// The file that tells the new version which old copy to remove (in the app's support folder).
pub fn marker_path(support: &Path) -> PathBuf {
    support.join("update-cleanup.txt")
}

/// At quit: swaps the staged version in, leaves a note for the new version to remove the old one
/// (`support`: the app's support folder), and opens the new version when `relaunch`. If the swap
/// fails, nothing changes and the disk image opens in Finder.
pub fn finish(s: &Staged, relaunch: bool, support: Option<&Path>) {
    match swap(s) {
        Ok(old) => {
            log::info!("update: installed {} at {}", s.version, s.bundle.display());
            let noted = support.is_some_and(|dir| {
                std::fs::create_dir_all(dir).is_ok() && std::fs::write(marker_path(dir), format!("{}\n", old.display())).is_ok()
            });
            if !noted && let Err(e) = std::fs::remove_dir_all(&old) {
                log::warn!("update: couldn't remove {}: {e}", old.display());
            }
            if relaunch {
                reopen(&s.bundle);
            }
        }
        Err(e) => {
            log::error!("update: couldn't install {}: {e}", s.version);
            let _ = std::fs::remove_dir_all(&s.new_app);
            open_in_finder(&s.dmg);
        }
    }
}

/// Opens a new instance of the app at `bundle` (NSWorkspace, formerly `open -n`).
pub fn reopen(bundle: &Path) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSWorkspace, NSWorkspaceOpenConfiguration};
        use objc2_foundation::{NSString, NSURL};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&bundle.display().to_string()));
        let config = NSWorkspaceOpenConfiguration::configuration();
        config.setCreatesNewApplicationInstance(true);
        // The dry run's relaunched app runs a short tour and quits (test-hook builds only).
        #[cfg(feature = "update-test-hook")]
        if let Ok(args) = std::env::var("LM_UPDATE_TEST_RELAUNCH_ARGS") {
            use objc2_foundation::NSArray;
            let retained: Vec<_> = args.lines().map(NSString::from_str).collect();
            config.setArguments(&NSArray::from_retained_slice(&retained));
        }
        NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(&url, &config, None);
        log::info!("update: reopened {}", bundle.display());
        return;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        log::warn!("update: reopen is macOS-only");
    }
}

/// Opens the disk image in Finder (the fallback when the app can't replace itself).
pub fn open_in_finder(dmg: &Path) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::{NSString, NSURL};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&dmg.display().to_string()));
        if !NSWorkspace::sharedWorkspace().openURL(&url) {
            log::warn!("update: couldn't open {}", dmg.display());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = dmg;
    }
}

/// At launch: removes the old version an update left behind (named in the marker in `support`)
/// and the downloaded disk images in `downloads`. Returns the removed copy, if any.
pub fn after_update(support: &Path, downloads: Option<&Path>) -> Option<PathBuf> {
    let marker = marker_path(support);
    let text = std::fs::read_to_string(&marker).ok()?;
    let _ = std::fs::remove_file(&marker);
    if let Some(d) = downloads {
        let _ = std::fs::remove_dir_all(d);
    }
    let old = PathBuf::from(text.lines().next()?.trim());
    // Only a copy this module moved aside, never anything else the file might name.
    if !is_old_copy(&old) || !old.is_dir() {
        return None;
    }
    match std::fs::remove_dir_all(&old) {
        Ok(()) => Some(old),
        Err(e) => {
            log::warn!("update: couldn't remove {}: {e}", old.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_of_an_executable() {
        assert_eq!(
            bundle_of(Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap")),
            Some(PathBuf::from("/Applications/NimbleMap.app"))
        );
        assert_eq!(bundle_of(Path::new("/Users/me/target/release/layeredmap")), None);
        assert_eq!(bundle_of(Path::new("/x/NimbleMap/Contents/MacOS/layeredmap")), None);
        assert_eq!(placement(None), Err(Manual::NotABundle));
    }

    #[test]
    fn translocated_apps_are_not_replaced() {
        let p = Path::new("/private/var/folders/xy/T/AppTranslocation/1234-ABCD/d/NimbleMap.app");
        assert_eq!(path_problem(p), Some(Manual::Translocated));
        assert_eq!(placement(Some(p)), Err(Manual::Translocated));
        assert_eq!(path_problem(Path::new("/Applications/NimbleMap.app")), None);
    }

    #[test]
    fn only_the_owners_team_is_accepted() {
        assert!(team_allowed(Some("75MXK7UJ6W")));
        assert!(!team_allowed(Some("ABCDE12345")));
        assert!(!team_allowed(Some("")));
        // Ad-hoc signatures (no team) pass in tests only, never in a release build.
        assert_eq!(team_allowed(None), cfg!(test) || cfg!(feature = "update-test-hook"));
    }

    #[test]
    fn only_old_copies_are_ever_removed() {
        assert!(is_old_copy(Path::new("/Applications/.NimbleMap-old-123")));
        assert!(!is_old_copy(Path::new("/Applications/NimbleMap.app")));
        assert!(!is_old_copy(Path::new("/Applications/.NimbleMap-update-123")));
        assert!(!is_old_copy(Path::new("/Applications/.NimbleMap-old-")));
        assert!(!is_old_copy(Path::new("/Applications/.NimbleMap-old-12x")));
        assert!(!is_old_copy(Path::new("/Users/me")));
        assert!(!is_old_copy(Path::new("/")));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cant_be_written_is_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("lm-update-ro-{}", std::process::id()));
        let app = dir.join("NimbleMap.app");
        std::fs::create_dir_all(&app).unwrap();
        assert_eq!(placement(Some(&app)), Ok(()));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root can write anywhere; the check means something only for an ordinary user.
        let root = std::fs::create_dir(dir.join("x")).is_ok();
        if !root {
            assert_eq!(placement(Some(&app)), Err(Manual::NotWritable));
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_marker_removes_only_the_old_copy() {
        let dir = std::env::temp_dir().join(format!("lm-update-marker-{}", std::process::id()));
        let support = dir.join("support");
        let downloads = dir.join("downloads");
        let old = dir.join(".NimbleMap-old-42");
        for d in [&support, &downloads, &old] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(marker_path(&support), format!("{}\n", old.display())).unwrap();
        assert_eq!(after_update(&support, Some(&downloads)), Some(old.clone()));
        assert!(!old.exists() && !downloads.exists() && !marker_path(&support).exists());
        // Nothing to do without a marker; a marker naming anything else removes nothing.
        assert_eq!(after_update(&support, None), None);
        std::fs::write(marker_path(&support), format!("{}\n", support.display())).unwrap();
        assert_eq!(after_update(&support, None), None);
        assert!(support.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Writes a minimal ad-hoc-signed app bundle.
    #[cfg(target_os = "macos")]
    fn fake_app(at: &Path, id: &str, version: &str) {
        std::fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
        std::fs::write(
            at.join("Contents/Info.plist"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>{id}</string>
  <key>CFBundleExecutable</key><string>layeredmap</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
</dict></plist>
"#
            ),
        )
        .unwrap();
        let exe = at.join("Contents/MacOS/layeredmap");
        std::fs::copy("/usr/bin/true", &exe).unwrap();
        let ok = Command::new("/usr/bin/codesign").args(["--force", "--sign", "-"]).arg(at).output().unwrap().status.success();
        assert!(ok, "codesign of the fake app failed");
    }

    /// A disk image holding `app` beside an Applications link, as `build-dmg.sh` makes; `None`
    /// when hdiutil can't make one here.
    #[cfg(target_os = "macos")]
    fn fake_dmg(dir: &Path, id: &str, version: &str) -> Option<PathBuf> {
        let stage = dir.join(format!("stage-{id}-{version}"));
        fake_app(&stage.join("NimbleMap.app"), id, version);
        std::os::unix::fs::symlink("/Applications", stage.join("Applications")).unwrap();
        let dmg = dir.join(format!("{id}-{version}.dmg"));
        let st = Command::new("/usr/bin/hdiutil")
            .args(["create", "-quiet", "-volname", "NimbleMap", "-fs", "HFS+", "-format", "UDZO", "-ov", "-srcfolder"])
            .arg(&stage)
            .arg(&dmg)
            .status()
            .ok()?;
        st.success().then_some(dmg)
    }

    /// The whole install on a temporary folder: a fake installed app, a disk image of a newer one
    /// built with hdiutil, staging, a wrong version refused, the swap, and the cleanup at the next
    /// launch.
    #[cfg(target_os = "macos")]
    #[test]
    fn installs_from_a_real_disk_image() {
        let dir = std::env::temp_dir().join(format!("lm-update-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let apps = dir.join("Applications");
        let bundle = apps.join("NimbleMap.app");
        fake_app(&bundle, "app.layeredmap.desktop", "1.0.0");
        let Some(dmg) = fake_dmg(&dir, "app.layeredmap.desktop", "1.1.0") else {
            eprintln!("hdiutil create is unavailable here: skipping the disk image test");
            return;
        };
        assert_eq!(placement(Some(&bundle)), Ok(()));

        // A disk image that holds another version is refused, and leaves nothing behind.
        let e = stage(&dmg, &bundle, "1.2.0").unwrap_err();
        assert!(e.contains("1.1.0"), "{e}");
        assert_eq!(std::fs::read_dir(&apps).unwrap().count(), 1);
        // So is another app's.
        let Some(other) = fake_dmg(&dir, "org.example.other", "1.1.0") else { return };
        assert!(stage(&other, &bundle, "1.1.0").unwrap_err().contains("org.example.other"));

        let s = stage(&dmg, &bundle, "1.1.0").unwrap();
        assert!(s.new_app.starts_with(&apps) && s.new_app.file_name().unwrap().to_string_lossy().starts_with(".NimbleMap-update-"));
        assert_eq!(plist_value(&s.new_app, "CFBundleShortVersionString").unwrap(), "1.1.0");
        // Ad-hoc signed in the test: no team (allowed only in tests and the dry run).
        assert_eq!(team_id(&s.new_app).unwrap(), None);
        // Still the old version in place until quit; the image is detached.
        assert_eq!(plist_value(&bundle, "CFBundleShortVersionString").unwrap(), "1.0.0");
        let mounts = run("/usr/bin/hdiutil", &["info".as_ref()]).unwrap();
        assert!(!mounts.contains(&format!("nimblemap-update-mount-{}", std::process::id())), "{mounts}");
        // The copy carries no quarantine attribute.
        let xattrs = run("/usr/bin/xattr", &[s.new_app.as_os_str()]).unwrap();
        assert!(!xattrs.contains("com.apple.quarantine"), "{xattrs}");

        let support = dir.join("support");
        finish(&s, false, Some(&support));
        assert_eq!(plist_value(&bundle, "CFBundleShortVersionString").unwrap(), "1.1.0");
        run("/usr/bin/codesign", &["--verify".as_ref(), "--strict".as_ref(), bundle.as_os_str()]).unwrap();
        let aside: Vec<_> = std::fs::read_dir(&apps).unwrap().flatten().map(|e| e.path()).filter(|p| is_old_copy(p)).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(plist_value(&aside[0], "CFBundleShortVersionString").unwrap(), "1.0.0");
        // The new version's launch removes the old one.
        assert_eq!(after_update(&support, None).as_deref(), Some(aside[0].as_path()));
        let left: Vec<_> = std::fs::read_dir(&apps).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left, vec![std::ffi::OsString::from("NimbleMap.app")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
