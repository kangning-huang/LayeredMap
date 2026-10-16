//! The question log (Assumptions and questions › Question budget instrumentation): one JSON line
//! per answered question card, kept locally, exportable as CSV for pilot analysis. It never
//! holds file names or data values (see `QuestionEvent`).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use lm_core::question::QuestionEvent;

use crate::error::{DocError, Result};

/// An append-only log of question events at a given path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestionLog {
    path: PathBuf,
}

impl QuestionLog {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        QuestionLog { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one event as a JSON line, creating the file (and its folder) if needed.
    pub fn append(&self, event: &QuestionEvent) -> Result<()> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| DocError::write(dir, e))?;
        }
        let mut line = serde_json::to_string(event)
            .map_err(|e| DocError::write(&self.path, std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
        line.push('\n');
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path).map_err(|e| DocError::write(&self.path, e))?;
        f.write_all(line.as_bytes()).map_err(|e| DocError::write(&self.path, e))
    }

    /// Every event in the log, oldest first. A missing log is empty; unreadable lines (a torn
    /// last write) are skipped.
    pub fn read_all(&self) -> Result<Vec<QuestionEvent>> {
        read_events(&self.path)
    }
}

fn read_events(path: &Path) -> Result<Vec<QuestionEvent>> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(DocError::read(path, e)),
    };
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line.map_err(|e| DocError::read(path, e))?;
        if let Ok(ev) = serde_json::from_str::<QuestionEvent>(&line) {
            out.push(ev);
        }
    }
    Ok(out)
}

/// Writes the log at `path_in` as CSV to `path_out` (atomically), with the header
/// `at_unix_ms,kind,answer_id,seconds_to_answer`. Returns the number of rows.
pub fn export_csv(path_in: &Path, path_out: &Path) -> Result<usize> {
    let events = read_events(path_in)?;
    let mut csv = String::from("at_unix_ms,kind,answer_id,seconds_to_answer\n");
    for e in &events {
        let kind = format!("{:?}", e.kind);
        csv.push_str(&format!("{},{},{},{}\n", e.at_unix_ms, kind, csv_field(&e.answer_id), e.seconds_to_answer));
    }
    rustgis_data::io::write_atomic(path_out, csv.as_bytes()).map_err(|e| DocError::write(path_out, e))?;
    Ok(events.len())
}

/// Quotes a CSV field when it holds a comma, quote or line break.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use lm_core::model::{AssumptionKind, Choice};

    #[test]
    fn appends_reads_and_exports() {
        let t = TempDir::new("qlog");
        let log = QuestionLog::new(t.0.join("logs").join("questions.jsonl"));
        assert_eq!(log.read_all().unwrap(), vec![]);
        let a = QuestionEvent::new(1_760_000_000_000, AssumptionKind::NoCrsLonLat, &Choice::new("wgs84", "WGS 84"), 4.25);
        let b = QuestionEvent::new(1_760_000_100_000, AssumptionKind::RasterCellMeaning, &Choice::not_sure(), 12.0);
        log.append(&a).unwrap();
        log.append(&b).unwrap();
        // A torn line is skipped, not fatal.
        std::fs::OpenOptions::new().append(true).open(log.path()).unwrap().write_all(b"{\"at_unix").unwrap();
        assert_eq!(log.read_all().unwrap(), vec![a, b]);

        let out = t.0.join("questions.csv");
        assert_eq!(export_csv(log.path(), &out).unwrap(), 2);
        let csv = std::fs::read_to_string(&out).unwrap();
        assert_eq!(
            csv,
            "at_unix_ms,kind,answer_id,seconds_to_answer\n\
             1760000000000,NoCrsLonLat,wgs84,4.25\n\
             1760000100000,RasterCellMeaning,not_sure,12\n"
        );
    }

    #[test]
    fn log_lines_hold_no_file_names_or_values() {
        let t = TempDir::new("qpriv");
        let log = QuestionLog::new(t.0.join("q.jsonl"));
        let e = QuestionEvent::new(1, AssumptionKind::CsvCoordinateColumns, &Choice::new("columns:lon,lat", "lon and lat"), 2.0);
        log.append(&e).unwrap();
        let text = std::fs::read_to_string(log.path()).unwrap();
        assert!(!text.contains("lon"), "{text}");
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["answer_id", "at_unix_ms", "kind", "seconds_to_answer"]);
    }

    #[test]
    fn csv_fields_are_quoted_when_needed() {
        assert_eq!(csv_field("wgs84"), "wgs84");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
