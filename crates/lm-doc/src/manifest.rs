//! Document v0: `<dir>/manifest.json`, `{"format": "layeredmap", "version": 0, ...}` followed by
//! the document's fields. Sources are referenced by path and fingerprint (`SourceRef`), never
//! copied.
//!
//! Compatibility (Gate protocol › What makes rollback cheap): until Gate 5 a phase only adds
//! fields, and a document saved by Phase N must open in Phase N−1, so unknown fields are
//! ignored. A newer format version is refused with a message saying so.

use std::path::Path;

use lm_core::document::{DOCUMENT_VERSION, Document};
use serde::{Deserialize, Serialize};

use crate::error::{DocError, Result};

/// The `format` value of every LayeredMap manifest.
pub const FORMAT_NAME: &str = "layeredmap";
/// The manifest's file name inside a document folder.
pub const MANIFEST_FILE: &str = "manifest.json";
/// A manifest holds references and profiles, not data; anything larger is not ours.
const MAX_MANIFEST_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    /// The document's fields, `version` among them.
    #[serde(flatten)]
    document: Document,
}

/// Writes `doc` to `<dir>/manifest.json` atomically (a temporary file, then a rename), creating
/// `dir` if needed. The manifest records this build's format version.
pub fn save(doc: &Document, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| DocError::write(dir, e))?;
    let path = dir.join(MANIFEST_FILE);
    let mut document = doc.clone();
    document.version = DOCUMENT_VERSION;
    let manifest = Manifest { format: FORMAT_NAME.into(), document };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| DocError::write(&path, std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    rustgis_data::io::write_atomic(&path, &bytes).map_err(|e| DocError::write(&path, e))
}

/// Reads `<dir>/manifest.json`. Unknown fields are ignored; a newer format version is refused.
pub fn load(dir: &Path) -> Result<Document> {
    let path = dir.join(MANIFEST_FILE);
    let len = std::fs::metadata(&path).map_err(|e| DocError::read(&path, e))?.len();
    if len > MAX_MANIFEST_BYTES {
        return Err(DocError::TooLarge { path: path.display().to_string() });
    }
    let bytes = std::fs::read(&path).map_err(|e| DocError::read(&path, e))?;
    parse(&bytes, &path)
}

fn parse(bytes: &[u8], path: &Path) -> Result<Document> {
    let shown = || path.display().to_string();
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| DocError::NotADocument { path: shown() })?;
    if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT_NAME) {
        return Err(DocError::NotADocument { path: shown() });
    }
    let version = value.get("version").and_then(|v| v.as_u64()).ok_or(DocError::NotADocument { path: shown() })?;
    if version > u64::from(DOCUMENT_VERSION) {
        return Err(DocError::TooNew { found: version, supported: DOCUMENT_VERSION });
    }
    let manifest: Manifest = serde_json::from_value(value).map_err(|e| DocError::Damaged { path: shown(), detail: e.to_string() })?;
    Ok(manifest.document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use lm_core::model::*;

    fn profile(file: &str, kind: LayerKind) -> LayerProfile {
        LayerProfile {
            source: SourceRef {
                path: format!("/data/{file}"),
                member: None,
                size: 1234,
                mtime: 1_700_000_000,
                sha256: Some("ab".repeat(32)),
            },
            format: "CSV".into(),
            kind,
            count: 248,
            extent_wgs84: Some(Extent { min_x: 121.0, min_y: 31.0, max_x: 122.0, max_y: 31.5 }),
            crs: Known::likely(Crs::Wgs84, "lon/lat columns"),
            fields: vec![FieldProfile {
                name: "enrolment".into(),
                dtype: DataType::Integer,
                role: Known::likely(FieldRole::Measure(MeasureKind::Count), "integer counts"),
                unit: Known::unknown(None, ""),
                stats: Some(FieldStats { min: Some(1.0), max: Some(900.0), median: Some(300.0), distinct: 200, nulls: 2 }),
            }],
            bands: vec![],
            assumptions: vec![Assumption {
                kind: AssumptionKind::CsvCoordinateColumns,
                subject: Subject::Layer,
                chosen: Choice::new("columns:lon,lat", "lon and lat"),
                alternatives: vec![Choice::new("columns:lon,lat", "lon and lat"), Choice::not_sure()],
                confidence: Confidence::Likely,
                consequence: Consequence::ChangesLocations,
                treatment: Treatment::Visible,
                line: "Located using lon and lat as GPS coordinates (WGS 84).".into(),
                confirmed_by_user: true,
            }],
            exclusions: vec![Exclusion { row: 7, reason: "No coordinates".into() }],
            encoding: Some("UTF-8".into()),
            notices: vec![],
            data_rev: 0,
        }
    }

    fn sample() -> Document {
        let mut d = Document::new();
        for (f, k) in [("districts.shp", LayerKind::Areas), ("schools.csv", LayerKind::Points)] {
            let tx = d.add_layer_tx(profile(f, k));
            d.apply(&tx).unwrap();
        }
        d.view = Some((121.47, 31.23, 11.5));
        d
    }

    #[test]
    fn save_then_load_round_trips() {
        let t = TempDir::new("roundtrip");
        let dir = t.0.join("My map");
        let d = sample();
        save(&d, &dir).unwrap();
        assert_eq!(load(&dir).unwrap(), d);
        let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["format"], "layeredmap");
        assert_eq!(v["version"], 0);
        // Saving again replaces the file and leaves no temporary files behind.
        save(&d, &dir).unwrap();
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let t = TempDir::new("unknown");
        let d = sample();
        save(&d, &t.0).unwrap();
        let p = t.0.join(MANIFEST_FILE);
        let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        v["notes"] = serde_json::json!([{"text": "from Phase 5"}]);
        v["layers"][0]["recipe"] = serde_json::json!({"steps": []});
        v["layers"][0]["style"]["classes"] = serde_json::json!(5);
        v["layers"][1]["profile"]["source"]["inode"] = serde_json::json!(42);
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(load(&t.0).unwrap(), d);
    }

    #[test]
    fn a_newer_format_is_refused_with_a_clear_message() {
        let t = TempDir::new("newer");
        save(&sample(), &t.0).unwrap();
        let p = t.0.join(MANIFEST_FILE);
        let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        v["version"] = serde_json::json!(99);
        std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
        let e = load(&t.0).unwrap_err();
        assert!(matches!(e, DocError::TooNew { found: 99, supported: 0 }));
        assert!(e.to_string().contains("newer version of LayeredMap"), "{e}");
    }

    #[test]
    fn junk_missing_and_damaged_manifests_are_errors() {
        let t = TempDir::new("junk");
        assert!(matches!(load(&t.0), Err(DocError::Read { .. })));
        let p = t.0.join(MANIFEST_FILE);
        std::fs::write(&p, b"not json").unwrap();
        assert!(matches!(load(&t.0), Err(DocError::NotADocument { .. })));
        std::fs::write(&p, br#"{"format":"other","version":0}"#).unwrap();
        assert!(matches!(load(&t.0), Err(DocError::NotADocument { .. })));
        std::fs::write(&p, br#"{"format":"layeredmap","version":0,"layers":"x"}"#).unwrap();
        assert!(matches!(load(&t.0), Err(DocError::Damaged { .. })));
        std::fs::write(&p, br#"{"format":"layeredmap","version":0,"layers":[],"next_layer_id":1}"#).unwrap();
        assert_eq!(load(&t.0).unwrap(), Document::new());
    }
}
