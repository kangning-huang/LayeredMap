//! Speaking a request into the Ask box (WP7.1): the bundle's `lm-dictate` helper
//! (`packaging/lm/dictate.swift`) listens with Apple's speech recognizer and writes what it hears,
//! one line per event, which this reads on a thread. Closing its input stops it. The words appear
//! in the field as they're recognised and stay editable before the request is sent.
//!
//! Windows (WP6.2) has no helper yet: the button says so and points to Windows' own voice typing
//! (Windows+H), which types into the focused Ask box like a keyboard.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use lm_core::{i18n, tl, tn};

/// What the microphone button says on Windows (in English; shown translated).
pub const WINDOWS: &str = tn!("Speaking isn't available on Windows yet. Press Windows+H to type with your voice instead.");

pub enum Event {
    Ready,
    Partial(String),
    Final(String),
    Error(String),
}

pub struct Dictation {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Event>,
}

/// The helper next to the app's executable (`Contents/MacOS/lm-dictate`), or `LM_DICTATE`.
fn helper() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LM_DICTATE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let p = std::env::current_exe().ok()?.parent()?.join("lm-dictate");
    p.is_file().then_some(p)
}

impl Dictation {
    /// Starts listening; `wake` repaints the app when words arrive.
    pub fn start(wake: impl Fn() + Send + 'static) -> Result<Dictation, String> {
        if cfg!(windows) && std::env::var_os("LM_DICTATE").is_none() {
            return Err(tl!(WINDOWS).into());
        }
        let path = helper().ok_or_else(|| {
            i18n::fmt("Speaking needs the {app} app (its speech helper isn't here).", &[("app", lm_core::brand::shown(true))])
        })?;
        let mut child = Command::new(path)
            // Offline, the helper refuses to fall back to Apple's server recognizer.
            .env("LM_OFFLINE", if lm_net::offline() { "1" } else { "0" })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| i18n::fmt("Couldn't start listening: {error}", &[("error", &e.to_string())]))?;
        let stdout = child.stdout.take().ok_or_else(|| tl!("Couldn't start listening.").to_string())?;

        let stdin = child.stdin.take();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let (kind, text) = line.split_once('\t').unwrap_or((line.as_str(), ""));
                let e = match kind {
                    "ready" => Event::Ready,
                    "partial" => Event::Partial(text.to_string()),
                    "final" => Event::Final(text.to_string()),
                    "error" => Event::Error(text.to_string()),
                    _ => continue,
                };
                let done = matches!(e, Event::Final(_) | Event::Error(_));
                if tx.send(e).is_err() {
                    break;
                }
                wake();
                if done {
                    break;
                }
            }
        });
        Ok(Dictation { child, stdin, rx })
    }

    /// Stops listening; the last words still arrive as `Final`.
    pub fn stop(&mut self) {
        self.stdin.take();
    }

    pub fn events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Dictation {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    /// Windows: the button explains instead of failing to start a helper.
    #[test]
    #[cfg(windows)]
    fn windows_points_to_voice_typing() {
        let r = super::Dictation::start(|| {});
        assert_eq!(r.err().as_deref(), Some(super::WINDOWS));
    }

    #[test]
    fn the_windows_line_names_the_shortcut() {
        assert!(super::WINDOWS.contains("Windows+H"));
    }
}
