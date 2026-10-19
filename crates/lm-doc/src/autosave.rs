//! Autosave and the recovery journal (WP5.1, WP5.2).
//!
//! The app autosaves every 30 seconds and when it loses focus, into its own folder
//! (`~/Library/Application Support/LayeredMap/Autosave/`), never over the user's file: that changes
//! only on ⌘S. A journal in the same folder (`session.json`) says whether the session is still
//! running and which autosave holds its unsaved work. A clean quit marks it clean and removes
//! the autosave; if the next launch finds it unclean with an autosave, it offers to restore it.
//!
//! The journal keeps an unanswered offer until the user answers it, so a second crash before
//! Restore or Discard loses nothing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::container::now_secs;
use crate::error::{DocError, Result};

/// The journal's file name in the autosave folder.
pub const JOURNAL_FILE: &str = "session.json";

/// The autosave folder under a home folder (macOS).
pub fn default_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/LayeredMap/Autosave")
}

/// An autosave the journal points to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutosaveEntry {
    /// The document's name ("Heat study", "Untitled").
    pub name: String,
    /// The autosave's file name in the autosave folder.
    pub file: String,
    /// Seconds since the Unix epoch.
    pub saved_at: i64,
    /// The user's own file, if the document has one.
    #[serde(default)]
    pub document: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct JournalState {
    pid: u32,
    started_at: i64,
    clean: bool,
    /// This session's autosave.
    #[serde(default)]
    autosave: Option<AutosaveEntry>,
    /// A crashed session's autosave not yet restored or discarded.
    #[serde(default)]
    offered: Option<AutosaveEntry>,
}

/// What the launch after a crash offers to restore.
#[derive(Clone, Debug, PartialEq)]
pub struct Recovery {
    pub name: String,
    pub saved_at: i64,
    /// The autosave file.
    pub autosave: PathBuf,
    /// The user's own file, if the document had one.
    pub document: Option<PathBuf>,
}

impl Recovery {
    /// "NimbleMap quit unexpectedly. Restore 'Heat study' from 2 minutes ago?"
    pub fn question(&self, app: &str, now: i64) -> String {
        format!("{app} quit unexpectedly. Restore '{}' from {}?", self.name, ago(now - self.saved_at))
    }
}

/// The recovery journal of the running session.
pub struct Journal {
    dir: PathBuf,
    state: JournalState,
}

impl Journal {
    /// Starts this session's journal in `dir`. If the last session did not end cleanly and left
    /// an autosave (or an offer still unanswered), returns it to offer.
    pub fn begin(dir: &Path) -> Result<(Journal, Option<Recovery>)> {
        std::fs::create_dir_all(dir).map_err(|e| DocError::write(dir, e))?;
        let previous: Option<JournalState> = std::fs::read(dir.join(JOURNAL_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let mut offered = None;
        if let Some(p) = previous {
            // An offer still open wins over the crashed session's own autosave, which is newer
            // only if the user worked on after a restore (then it is the one to offer).
            let crashed = if p.clean { None } else { p.autosave.clone() };
            offered = crashed.or(p.offered.clone()).filter(|e| dir.join(&e.file).is_file());
            // The older of two candidates is no longer offered.
            for e in [p.autosave, p.offered].into_iter().flatten() {
                if offered.as_ref().is_none_or(|o| o.file != e.file) {
                    let _ = std::fs::remove_file(dir.join(&e.file));
                }
            }
        }
        let state =
            JournalState { pid: std::process::id(), started_at: now_secs(), clean: false, autosave: None, offered: offered.clone() };
        let journal = Journal { dir: dir.to_path_buf(), state };
        journal.write()?;
        let recovery = offered.map(|e| Recovery {
            name: e.name.clone(),
            saved_at: e.saved_at,
            autosave: dir.join(&e.file),
            document: e.document.as_ref().map(PathBuf::from),
        });
        Ok((journal, recovery))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where this session autosaves.
    pub fn autosave_path(&self) -> PathBuf {
        self.dir.join(format!("autosave-{}-{}.lmap", self.state.started_at, self.state.pid))
    }

    /// Records an autosave just written to [`Journal::autosave_path`].
    pub fn autosaved(&mut self, name: &str, document: Option<&Path>) -> Result<()> {
        let file = self.autosave_path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        self.state.autosave =
            Some(AutosaveEntry { name: name.to_string(), file, saved_at: now_secs(), document: document.map(|p| p.display().to_string()) });
        self.write()
    }

    /// The document has nothing unsaved (just saved, or back as it was): its autosave goes.
    pub fn nothing_unsaved(&mut self) -> Result<()> {
        if let Some(e) = self.state.autosave.take() {
            let _ = std::fs::remove_file(self.dir.join(e.file));
            self.write()?;
        }
        Ok(())
    }

    /// The offer was answered (Restore or Discard): the crashed session's autosave goes. Restored
    /// work is autosaved again by this session.
    pub fn answered(&mut self) -> Result<()> {
        if let Some(e) = self.state.offered.take() {
            let _ = std::fs::remove_file(self.dir.join(e.file));
        }
        self.write()
    }

    /// A clean quit: the session's autosave goes and the journal is marked clean. An unanswered
    /// offer stays for the next launch.
    pub fn end_clean(&mut self) -> Result<()> {
        if let Some(e) = self.state.autosave.take() {
            let _ = std::fs::remove_file(self.dir.join(e.file));
        }
        self.state.clean = true;
        self.write()
    }

    fn write(&self) -> Result<()> {
        let path = self.dir.join(JOURNAL_FILE);
        let bytes = serde_json::to_vec_pretty(&self.state).map_err(|e| DocError::write(&path, std::io::Error::other(e)))?;
        rustgis_data::io::write_atomic(&path, &bytes).map_err(|e| DocError::write(&path, e))
    }
}

/// "just now", "1 minute ago", "2 minutes ago", "3 hours ago", "yesterday", "4 days ago".
pub fn ago(secs: i64) -> String {
    let s = secs.max(0);
    let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
    match s {
        0..60 => "just now".into(),
        60..3600 => plural(s / 60, "minute"),
        3600..86_400 => plural(s / 3600, "hour"),
        86_400..172_800 => "yesterday".into(),
        _ => plural(s / 86_400, "day"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::{OwnedData, load_lmap, save_lmap};
    use crate::testutil::TempDir;
    use lm_core::document::Document;

    fn touch(j: &Journal) {
        std::fs::write(j.autosave_path(), b"autosave").unwrap();
    }

    #[test]
    fn a_clean_quit_offers_nothing_and_removes_the_autosave() {
        let t = TempDir::new("journal-clean");
        let (mut j, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_none());
        touch(&j);
        j.autosaved("Heat study", None).unwrap();
        let path = j.autosave_path();
        j.end_clean().unwrap();
        assert!(!path.exists());
        let (_, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn a_crash_offers_the_autosave_until_it_is_answered() {
        let t = TempDir::new("journal-crash");
        let (mut j, _) = Journal::begin(&t.0).unwrap();
        touch(&j);
        j.autosaved("Heat study", Some(Path::new("/Users/me/Heat study.lmap"))).unwrap();
        let path = j.autosave_path();
        drop(j); // the process dies: no end_clean
        let (mut j2, r) = Journal::begin(&t.0).unwrap();
        let r = r.unwrap();
        assert_eq!(r.name, "Heat study");
        assert_eq!(r.autosave, path);
        assert_eq!(r.document.as_deref(), Some(Path::new("/Users/me/Heat study.lmap")));
        // It crashes again before the user answers: still offered.
        drop(j2);
        let (mut j3, r) = Journal::begin(&t.0).unwrap();
        assert_eq!(r.as_ref().map(|r| r.autosave.clone()), Some(path.clone()));
        // A clean quit without an answer keeps the offer too.
        j3.end_clean().unwrap();
        let (j4, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_some());
        j2 = j4;
        j2.answered().unwrap();
        assert!(!path.exists());
        drop(j2);
        let (_, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn saving_removes_the_autosave_so_nothing_is_offered() {
        let t = TempDir::new("journal-saved");
        let (mut j, _) = Journal::begin(&t.0).unwrap();
        touch(&j);
        j.autosaved("Untitled", None).unwrap();
        j.nothing_unsaved().unwrap();
        assert!(!j.autosave_path().exists());
        drop(j);
        let (_, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn work_after_a_restore_is_what_the_next_crash_offers() {
        let t = TempDir::new("journal-after");
        let (mut j, _) = Journal::begin(&t.0).unwrap();
        touch(&j);
        j.autosaved("A", None).unwrap();
        let first = j.autosave_path();
        drop(j);
        std::thread::sleep(std::time::Duration::from_millis(1100)); // a new session name
        let (mut j, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_some());
        j.answered().unwrap();
        touch(&j);
        j.autosaved("A", None).unwrap();
        let second = j.autosave_path();
        assert_ne!(first, second);
        drop(j);
        let (_, r) = Journal::begin(&t.0).unwrap();
        assert_eq!(r.unwrap().autosave, second);
        assert!(!first.exists());
    }

    /// Autosave restore: what was autosaved before a crash is what the next launch restores.
    #[test]
    fn an_autosaved_document_is_restored_after_a_crash() {
        let t = TempDir::new("journal-restore");
        let (mut j, _) = Journal::begin(&t.0).unwrap();
        let mut d = Document::new();
        d.view = Some((1.0, 2.0, 3.0));
        let owned: Vec<(lm_core::model::LayerId, OwnedData)> = Vec::new();
        save_lmap(&d, &owned, &j.autosave_path()).unwrap();
        j.autosaved("Heat study", None).unwrap();
        drop(j);
        let (_, r) = Journal::begin(&t.0).unwrap();
        let r = r.unwrap();
        assert_eq!(load_lmap(&r.autosave).unwrap().doc, d);
        assert!(
            r.question(lm_core::brand::NAME, r.saved_at + 125)
                .starts_with("NimbleMap quit unexpectedly. Restore 'Heat study' from 2 minutes ago?")
        );
    }

    #[test]
    fn a_damaged_journal_is_a_fresh_start() {
        let t = TempDir::new("journal-damaged");
        std::fs::write(t.0.join(JOURNAL_FILE), b"{oops").unwrap();
        let (_, r) = Journal::begin(&t.0).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn ago_reads_naturally() {
        assert_eq!(ago(-5), "just now");
        assert_eq!(ago(59), "just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(125), "2 minutes ago");
        assert_eq!(ago(3600), "1 hour ago");
        assert_eq!(ago(7300), "2 hours ago");
        assert_eq!(ago(90_000), "yesterday");
        assert_eq!(ago(4 * 86_400), "4 days ago");
        assert_eq!(default_dir(Path::new("/Users/me")), PathBuf::from("/Users/me/Library/Application Support/LayeredMap/Autosave"));
    }
}
