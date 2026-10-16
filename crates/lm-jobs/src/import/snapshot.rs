//! Profile snapshots (Gate 2 › Automated: "its profile matches the approved JSON snapshot:
//! counts, field roles, coordinate system, assumptions"). A compact, deterministic summary of an
//! import's result: no absolute paths, no modification times, extents rounded to 6 decimals, so
//! the same file gives the same JSON on every machine. The approved snapshots live in
//! `fixtures/snapshots/<ID>.json`; the fixture tests compare against them.

use serde_json::{Value, json};

use super::{ImportError, ImportedLayer};

/// Rounds to 6 decimal places (about 10 cm in degrees).
fn round6(v: f64) -> Value {
    let r = (v * 1e6).round() / 1e6;
    // -0.0 and 0.0 print alike.
    let r = if r == 0.0 { 0.0 } else { r };
    serde_json::Number::from_f64(r).map_or(Value::Null, Value::Number)
}

fn opt6(v: Option<f64>) -> Value {
    v.map_or(Value::Null, round6)
}

/// One layer's summary.
pub fn layer_summary(l: &ImportedLayer) -> Value {
    let p = &l.profile;
    let fields: serde_json::Map<String, Value> =
        p.fields.iter().map(|f| (f.name.clone(), Value::String(format!("{:?} ({:?})", f.role.value, f.role.confidence)))).collect();
    let assumptions: Vec<Value> = p
        .assumptions
        .iter()
        .map(|a| {
            json!({
                "kind": format!("{:?}", a.kind),
                "treatment": format!("{:?}", a.treatment),
                "chosen": a.chosen.id,
                "line": a.line,
            })
        })
        .collect();
    let bands: Vec<Value> = p
        .bands
        .iter()
        .map(|b| {
            json!({
                "index": b.index,
                "nodata": opt6(b.nodata.value),
                "scale": opt6(b.scale),
                "offset": opt6(b.offset),
                "unit": b.unit.value,
                "cell_meaning": format!("{:?} ({:?})", b.cell_meaning.value, b.cell_meaning.confidence),
            })
        })
        .collect();
    let mut out = json!({
        "name": l.name,
        "member": p.source.member,
        "format": p.format,
        "kind": format!("{:?}", p.kind),
        "count": p.count,
        "crs": format!("{:?}", p.crs.value),
        "crs_confidence": format!("{:?}", p.crs.confidence),
        "extent": p.extent_wgs84.map(|e| vec![round6(e.min_x), round6(e.min_y), round6(e.max_x), round6(e.max_y)]),
        "encoding": p.encoding,
        "fields": fields,
        "assumptions": assumptions,
        "exclusions": p.exclusions.len(),
        "notices": p.notices,
        "status": format!("{:?}", l.status),
    });
    if !bands.is_empty()
        && let Some(m) = out.as_object_mut()
    {
        m.insert("bands".into(), Value::Array(bands));
    }
    out
}

/// The summary of importing `file` (a name, never a path): its layers, or why it didn't open.
/// Technical details are left out (they can hold paths and library wording).
pub fn summary(file: &str, result: &Result<Vec<ImportedLayer>, ImportError>) -> Value {
    match result {
        Ok(layers) => json!({ "file": file, "layers": layers.iter().map(layer_summary).collect::<Vec<_>>() }),
        Err(e) => {
            let kind = match e {
                ImportError::Canceled => "Canceled",
                ImportError::Refused(_) => "Refused",
                ImportError::Unsupported(_) => "Unsupported",
                ImportError::Failed { .. } => "Failed",
            };
            json!({ "file": file, "error": { "kind": kind, "message": e.to_string() } })
        }
    }
}
