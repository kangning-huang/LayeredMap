//! Local diagnostics (WP5.7): crash logs and a small log file stay on this Mac, in
//! `~/Library/Logs/LayeredMap/` (where Console.app looks). Nothing is sent anywhere. Help ›
//! Export Diagnostics… assembles [`report`], the shell shows it for review, and only then is it
//! saved to a file the user picks and may send themselves.
//!
//! - A panic hook writes `crash-<time>.txt` (version, thread, message, location, backtrace) and
//!   then lets the default hook run. The newest [`KEEP_CRASHES`] are kept.
//! - A minimal logger appends warnings and errors (and the app's own info lines) to
//!   `layeredmap.log`, which is started afresh once it passes [`MAX_LOG`].

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Crash logs kept; older ones are deleted when a new one is written.
pub const KEEP_CRASHES: usize = 20;
/// The log file's size limit; past it, the file is moved to `layeredmap.old.log` and restarted.
pub const MAX_LOG: u64 = 1 << 20;
/// How much of the log a report includes.
const REPORT_LOG_LINES: usize = 300;
/// Crash logs a report includes, newest first.
const REPORT_CRASHES: usize = 3;

/// `~/Library/Logs/LayeredMap` (macOS); elsewhere a `LayeredMap/logs` folder in the home folder.
pub fn logs_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    if cfg!(target_os = "macos") { home.join("Library/Logs/LayeredMap") } else { home.join("LayeredMap/logs") }
}

/// Seconds since 1970 as `YYYY-MM-DD HH:MM:SS` (UTC).
fn timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let s = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The app and the machine, at the top of every crash log and report.
pub fn about(app: &str) -> String {
    format!(
        "{app} {} ({} build, {} {})\n{}\n",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) { "debug" } else { "release" },
        std::env::consts::OS,
        std::env::consts::ARCH,
        os_version().map_or_else(|| "system version unknown".to_string(), |v| format!("macOS {v}")),
    )
}

fn os_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/sw_vers").arg("-productVersion").output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// A crash log's text.
pub fn crash_text(header: &str, thread: &str, message: &str, location: &str, backtrace: &str, at: u64) -> String {
    format!(
        "LayeredMap crash log\n{header}time: {} UTC\nthread: {thread}\npanic: {message}\nat: {location}\n\nbacktrace:\n{backtrace}\n",
        timestamp(at)
    )
}

/// Deletes all but the newest `keep` crash logs in `dir`.
fn prune(dir: &Path, keep: usize) {
    let mut crashes = crash_logs(dir);
    if crashes.len() > keep {
        crashes.drain(..keep);
        for p in crashes {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The crash logs in `dir`, newest first (their names sort by time).
pub fn crash_logs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("crash-") && n.ends_with(".txt")))
        .collect();
    v.sort();
    v.reverse();
    v
}

/// Installs the panic hook: a crash log, then the default hook (which prints to stderr).
pub fn install_panic_hook(app: &'static str) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = logs_dir();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let location = info.location().map_or_else(|| "unknown".to_string(), |l| format!("{}:{}", l.file(), l.line()));
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let at = now();
        let text = crash_text(&about(app), &thread, &message, &location, &backtrace, at);
        if std::fs::create_dir_all(&dir).is_ok() {
            let name = format!("crash-{}.txt", timestamp(at).replace([' ', ':'], "-"));
            let _ = std::fs::write(dir.join(name), text);
            prune(&dir, KEEP_CRASHES);
        }
        log::error!("panic on thread {thread} at {location}: {message}");
        default(info);
    }));
}

/// Warnings and errors from everything, plus LayeredMap's own info lines, appended to a file.
struct FileLogger {
    path: PathBuf,
    file: Mutex<Option<std::fs::File>>,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Warn || (m.level() == log::Level::Info && is_ours(m.target()))
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!("{} {:<5} {}: {}\n", timestamp(now()), r.level(), r.target(), r.args());
        let mut f = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if f.as_ref().and_then(|f| f.metadata().ok()).is_some_and(|m| m.len() > MAX_LOG) {
            *f = None;
            let _ = std::fs::rename(&self.path, self.path.with_file_name("layeredmap.old.log"));
        }
        if f.is_none() {
            *f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(file) = f.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = f.flush();
        }
    }
}

fn is_ours(target: &str) -> bool {
    ["layeredmap", "lm_", "rustgis_tiles"].iter().any(|p| target.starts_with(p))
}

/// Starts logging to `layeredmap.log` (once; a logger set elsewhere wins).
pub fn install_logger() {
    let dir = logs_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let logger = Box::new(FileLogger { path: dir.join("layeredmap.log"), file: Mutex::new(None) });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

/// The last `n` lines of a text file.
fn tail(path: &Path, n: usize) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    Some(lines[lines.len().saturating_sub(n)..].join("\n"))
}

/// Settings that help explain a problem (no data, no file names).
pub struct Settings {
    pub app: &'static str,
    pub basemap: String,
    pub offline: bool,
    pub layers: usize,
}

/// The diagnostics report: the app and machine, settings, the network gate's counts, the log's
/// last lines and the newest crash logs from `dir`.
pub fn report(s: &Settings, dir: &Path) -> String {
    let mut out = format!("LayeredMap diagnostics\n{}created: {} UTC\n\n", about(s.app), timestamp(now()));
    out.push_str("== Settings ==\n");
    out.push_str(&format!("basemap: {}\noffline mode: {}\nlayers open: {}\n\n", s.basemap, if s.offline { "on" } else { "off" }, s.layers));
    out.push_str("== Network requests since launch ==\n");
    for l in lm_net::counts().lines() {
        out.push_str(&l);
        out.push('\n');
    }
    out.push_str(&format!("\n== Log (last {REPORT_LOG_LINES} lines of {}) ==\n", dir.join("layeredmap.log").display()));
    out.push_str(&tail(&dir.join("layeredmap.log"), REPORT_LOG_LINES).unwrap_or_else(|| "(no log yet)".into()));
    out.push_str("\n\n== Crash logs ==\n");
    let crashes = crash_logs(dir);
    if crashes.is_empty() {
        out.push_str("(none)\n");
    }
    for p in crashes.iter().take(REPORT_CRASHES) {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("crash");
        out.push_str(&format!("--- {name} ---\n{}\n", std::fs::read_to_string(p).unwrap_or_default()));
    }
    if crashes.len() > REPORT_CRASHES {
        out.push_str(&format!("({} older crash logs not included)\n", crashes.len() - REPORT_CRASHES));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_utc() {
        assert_eq!(timestamp(0), "1970-01-01 00:00:00");
        assert_eq!(timestamp(1_792_281_600 + 3661), "2026-10-18 01:01:01");
    }

    #[test]
    fn crash_logs_are_pruned_and_reported_newest_first() {
        let dir = std::env::temp_dir().join(format!("lm-diagnostics-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            let text = crash_text("LayeredMap 0.0.0\n", "main", &format!("boom {i}"), "src/x.rs:1", "0: frame", i);
            std::fs::write(dir.join(format!("crash-1970-01-01-00-00-0{i}.txt")), text).unwrap();
        }
        std::fs::write(dir.join("notes.txt"), "not a crash").unwrap();
        prune(&dir, 4);
        let left = crash_logs(&dir);
        assert_eq!(left.len(), 4);
        assert!(left[0].ends_with("crash-1970-01-01-00-00-04.txt"));
        std::fs::write(dir.join("layeredmap.log"), (0..400).map(|i| format!("line {i}\n")).collect::<String>()).unwrap();
        let s = Settings { app: "LayeredMap", basemap: "auto".into(), offline: true, layers: 2 };
        let r = report(&s, &dir);
        assert!(r.contains("offline mode: on") && r.contains("layers open: 2"));
        assert!(r.contains("panic: boom 4") && r.contains("panic: boom 2") && !r.contains("panic: boom 1"));
        assert!(r.contains("(1 older crash logs not included)"));
        assert!(r.contains("line 399") && !r.contains("line 99\n"));
        assert!(r.contains("assistant: "));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
