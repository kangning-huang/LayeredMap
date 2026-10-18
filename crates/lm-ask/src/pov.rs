//! A country point of view applied to the default countries (see `reference`). The manifest's
//! `pov.variants.<code>` holds `replace` (match key → shape id), `add` (shape ids) and `remove`
//! (match keys); the shapes are in `pov/shapes.geojson`, each with the country fields and its
//! `shape_id`. The match key of a country is its `iso_a3` when that is non-empty and unique in
//! the layer, else `iso_a3|name` (tools/lm/reference/build_reference.py; README "Points of view").

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value as Json;

use lm_jobs::import::{Ctx, Dataset, LayerData, import_path};

fn text(ds: &Dataset, row: usize, field: &str) -> String {
    ds.field_index(field).map(|k| ds.value(row, k).to_string()).filter(|v| v != "<Null>").unwrap_or_default()
}

/// Each feature's match key.
pub fn match_keys(ds: &Dataset) -> Vec<String> {
    let codes: Vec<String> = (0..ds.features.len()).map(|r| text(ds, r, "iso_a3")).collect();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for c in &codes {
        *seen.entry(c.as_str()).or_default() += 1;
    }
    codes
        .iter()
        .enumerate()
        .map(|(r, c)| if !c.is_empty() && seen[c.as_str()] == 1 { c.clone() } else { format!("{c}|{}", text(ds, r, "name")) })
        .collect()
}

pub fn apply(dir: &Path, manifest: &Json, pov: &str, ds: &Dataset) -> Result<Dataset, String> {
    let v = &manifest["pov"]["variants"][pov];
    if v.is_null() {
        return Err(format!("no view {pov} in the manifest"));
    }
    let strs = |j: &Json| -> Vec<String> { j.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
    let remove = strs(&v["remove"]);
    let replace: Vec<(String, String)> =
        v["replace"].as_object().into_iter().flatten().filter_map(|(k, s)| s.as_str().map(|s| (k.clone(), s.to_string()))).collect();
    let mut wanted: Vec<String> = replace.iter().map(|(_, s)| s.clone()).collect();
    wanted.extend(strs(&v["add"]));

    let path = dir.join(manifest["pov"]["shapes"].as_str().unwrap_or("pov/shapes.geojson"));
    let mut ls = import_path(&path, &Ctx::detached(Default::default())).map_err(|e| format!("{}: {e:?}", path.display()))?;
    let Some(LayerData::Vector(shapes)) = ls.drain(..).next().map(|l| l.data) else { return Err("no shapes".into()) };
    let sid = shapes.field_index("shape_id").ok_or("shapes have no shape_id")?;
    let by_id: HashMap<String, usize> = (0..shapes.features.len()).map(|r| (shapes.value(r, sid).to_string(), r)).collect();

    let keys = match_keys(ds);
    let drop: std::collections::HashSet<&str> = remove.iter().map(String::as_str).chain(replace.iter().map(|(k, _)| k.as_str())).collect();
    let mut out = ds.clone();
    out.features = ds.features.iter().zip(&keys).filter(|(_, k)| !drop.contains(k.as_str())).map(|(f, _)| f.clone()).collect();
    // The shapes' fields in the layer's order (shape_id left out).
    let map: Vec<Option<usize>> = ds.fields.iter().map(|f| shapes.field_index(&f.name)).collect();
    let first_id = ds.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    for (fid, id) in (first_id..).zip(&wanted) {
        let r = *by_id.get(id).ok_or_else(|| format!("shape {id} missing"))?;
        let src = &shapes.features[r];
        let mut f = src.clone();
        f.id = fid;
        f.attrs = map.iter().map(|k| k.map(|k| src.attrs.get(k).cloned().unwrap_or_default()).unwrap_or_default()).collect();
        out.features.push(f);
    }
    Ok(out)
}
