//! GeoJSON, streamed: a FeatureCollection's features are parsed one at a time as the file is
//! read, so a 500 MB file never sits in memory as one JSON tree. Mixed geometry is split into
//! points, lines and areas (catalog: "Split into 3 layers").

use lm_core::brand::NAME as APP;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Assumption, Exclusion};
use rustgis_engine::data::{Dataset, Field, FieldType, GeometryType, Value};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value as Json;

use super::limits::GEOJSON_MAX_BYTES;
use super::progress::Counting;
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, status_of, vector_profile};

pub struct GeoJson;

impl FormatAdapter for GeoJson {
    fn name(&self) -> &'static str {
        "GeoJSON"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        match extension(path).as_str() {
            "geojson" | "geojsonl" => true,
            "json" => {
                let t = String::from_utf8_lossy(head);
                t.contains("\"type\"") && !t.contains("\"Topology\"")
            }
            _ => false,
        }
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > GEOJSON_MAX_BYTES {
            return Err(ImportError::Refused(format!(
                "{} is larger than 2 GB, the most {APP} reads as GeoJSON. Converting it to GeoPackage or FlatGeobuf makes it open quickly. Your file has not been changed.",
                file_name(path)
            )));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        ctx.ticket.set_total(meta.size);
        let file = std::fs::File::open(path).map_err(|e| ImportError::read(&name, e))?;
        let reader = std::io::BufReader::with_capacity(1 << 20, Counting::new(file, ctx.ticket.clone()));
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
        let mut sink = Sink::new(stem);
        let mut de = serde_json::Deserializer::from_reader(reader);
        Top { sink: &mut sink }.deserialize(&mut de).map_err(|e| json_error(&name, e))?;
        ctx.ticket.check()?;
        ctx.stage(Stage::Extent)?;
        let (ds, exclusions) = sink.finish(path);
        ctx.stage(Stage::Profile)?;
        let parts = super::split_by_kind(ds);
        let kinds: Vec<_> = parts.iter().map(|(k, _)| *k).collect();
        let split = lm_profile::mixed_geometry_split(&kinds);
        let multi = parts.len() > 1;
        let mut out = Vec::new();
        for (i, (_, part)) in parts.into_iter().enumerate() {
            let mut assumptions: Vec<Assumption> = Vec::new();
            if let Some(a) = &split {
                assumptions.push(a.clone());
            }
            // Excluded records are reported once, on the first layer.
            let excl = if i == 0 { exclusions.clone() } else { Vec::new() };
            let member = multi.then(|| part.name.clone());
            let field_names: Vec<String> = part.fields.iter().map(|f| f.name.clone()).collect();
            let mut profile = vector_profile(path, member, "GeoJSON", &part, None, assumptions, excl, None, Vec::new(), &field_names);
            // GeoJSON is GPS coordinates (WGS 84) by definition (RFC 7946): when the values are
            // degrees, that is the format's rule, not a guess, so no "Assumed…" line. The China
            // offset notice still applies (Chinese map apps export offset GeoJSON too).
            if profile.crs.value == lm_core::model::Crs::Wgs84 {
                profile.assumptions.retain(|a| a.kind != lm_core::model::AssumptionKind::NoCrsLonLat);
                profile.crs.evidence = lm_core::model::Evidence("GeoJSON coordinates are GPS coordinates (WGS 84) by definition.".into());
            }
            out.push(ImportedLayer {
                name: part.name.clone(),
                status: status_of(&profile),
                profile,
                data: LayerData::Vector(Arc::new(part)),
                raw: None,
                style: None,
            });
        }
        Ok(out)
    }
}

fn json_error(name: &str, e: serde_json::Error) -> ImportError {
    let details = e.to_string();
    if super::progress::is_cancel_message(&details) {
        return ImportError::Canceled;
    }
    let message = if details.contains("recursion limit") {
        format!("{name} is nested too deeply to be valid GeoJSON. Your file has not been changed.")
    } else {
        format!("{name} isn't valid GeoJSON ({}). Your file has not been changed.", where_in(&e))
    };
    ImportError::Failed { message, details }
}

fn where_in(e: &serde_json::Error) -> String {
    format!("a problem on line {}", lm_core::model::group_thousands(e.line() as u64))
}

/// Collects features as they stream past: geometry, and properties into columns.
struct Sink {
    name: String,
    fields: Vec<String>,
    index: HashMap<String, usize>,
    types: Vec<Option<FieldType>>,
    rows: Vec<(Option<rustgis_engine::data::geometry::Geometry>, Vec<(usize, Value)>)>,
    exclusions: Vec<Exclusion>,
    seen: u64,
}

impl Sink {
    fn new(name: String) -> Self {
        Sink { name, fields: Vec::new(), index: HashMap::new(), types: Vec::new(), rows: Vec::new(), exclusions: Vec::new(), seen: 0 }
    }

    fn feature(&mut self, f: Json) {
        self.seen += 1;
        let row = self.seen;
        let (geometry, props) = match f {
            Json::Object(mut m) => match m.get("type").and_then(Json::as_str) {
                Some("Feature") => {
                    let geometry = m.remove("geometry").unwrap_or(Json::Null);
                    let mut props = m.remove("properties").unwrap_or(Json::Null);
                    // The Feature's own id is kept as a field (unless a property has that name).
                    if let Some(id) = m.remove("id") {
                        match &mut props {
                            Json::Object(p) => {
                                p.entry("id").or_insert(id);
                            }
                            Json::Null => props = Json::Object(serde_json::Map::from_iter([("id".to_string(), id)])),
                            _ => {}
                        }
                    }
                    (geometry, props)
                }
                Some(_) => (Json::Object(m), Json::Null),
                None => (Json::Null, Json::Null),
            },
            _ => (Json::Null, Json::Null),
        };
        let geom = if geometry.is_null() {
            self.exclusions.push(Exclusion { row, reason: "No geometry".into() });
            None
        } else {
            match rustgis_engine::data::io::geometry_from_geojson(&geometry) {
                Some(g) => {
                    let mut ok = true;
                    g.for_each_coord(|c| ok &= c.is_finite());
                    if ok {
                        Some(g)
                    } else {
                        self.exclusions.push(Exclusion { row, reason: "Coordinates that aren't numbers".into() });
                        None
                    }
                }
                None => {
                    self.exclusions.push(Exclusion { row, reason: format!("A geometry {APP} can't read") });
                    None
                }
            }
        };
        let mut attrs = Vec::new();
        if let Json::Object(p) = props {
            for (k, v) in p {
                let i = match self.index.get(&k) {
                    Some(i) => *i,
                    None => {
                        self.fields.push(k.clone());
                        self.types.push(None);
                        self.index.insert(k, self.fields.len() - 1);
                        self.fields.len() - 1
                    }
                };
                let (value, ty) = convert(v);
                if let (Some(slot), Some(ty)) = (self.types.get_mut(i), ty) {
                    *slot = Some(match *slot {
                        None => ty,
                        Some(a) if a == ty => a,
                        Some(FieldType::Integer) if ty == FieldType::Double => FieldType::Double,
                        Some(FieldType::Double) if ty == FieldType::Integer => FieldType::Double,
                        Some(_) => FieldType::Text,
                    });
                }
                attrs.push((i, value));
            }
        }
        self.rows.push((geom, attrs));
    }

    fn finish(self, path: &Path) -> (Dataset, Vec<Exclusion>) {
        let types: Vec<FieldType> = self.types.iter().map(|t| t.unwrap_or(FieldType::Text)).collect();
        let fields = self.fields.iter().zip(&types).map(|(n, t)| Field::new(n.clone(), *t)).collect();
        let mut ds = Dataset::new(self.name, GeometryType::None, fields);
        ds.source = Some(path.display().to_string());
        ds.features.reserve(self.rows.len());
        for (geom, attrs) in self.rows {
            let mut row = vec![Value::Null; types.len()];
            for (i, v) in attrs {
                if let (Some(slot), Some(t)) = (row.get_mut(i), types.get(i)) {
                    *slot = match (v, t) {
                        (Value::Int(n), FieldType::Double) => Value::Float(n as f64),
                        (Value::Int(n), FieldType::Text) => Value::Text(n.to_string()),
                        (Value::Float(n), FieldType::Text) => Value::Text(n.to_string()),
                        (Value::Bool(b), FieldType::Text) => Value::Text(b.to_string()),
                        (v, _) => v,
                    };
                }
            }
            ds.push_next(geom, row);
        }
        (ds, self.exclusions)
    }
}

fn convert(v: Json) -> (Value, Option<FieldType>) {
    match v {
        Json::Null => (Value::Null, None),
        Json::Bool(b) => (Value::Bool(b), Some(FieldType::Boolean)),
        Json::Number(n) => match n.as_i64() {
            Some(i) => (Value::Int(i), Some(FieldType::Integer)),
            None => (n.as_f64().map_or(Value::Null, Value::Float), Some(FieldType::Double)),
        },
        Json::String(s) => (Value::Text(s), Some(FieldType::Text)),
        other => (Value::Text(other.to_string()), Some(FieldType::Text)),
    }
}

/// The top-level object: `features` is streamed into the sink; anything else that's a single
/// Feature or geometry becomes one feature.
struct Top<'a> {
    sink: &'a mut Sink,
}

impl<'de> DeserializeSeed<'de> for Top<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Top<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a GeoJSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut other = serde_json::Map::new();
        let mut had_features = false;
        while let Some(key) = map.next_key::<String>()? {
            if key == "features" {
                had_features = true;
                map.next_value_seed(Features { sink: self.sink })?;
            } else if key == "crs" || key == "bbox" {
                map.next_value::<IgnoredAny>()?;
            } else {
                other.insert(key, map.next_value::<Json>()?);
            }
        }
        if !had_features {
            self.sink.feature(Json::Object(other));
        }
        Ok(())
    }

    // Newline-delimited or a bare array of features.
    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<(), A::Error> {
        Features { sink: self.sink }.visit_seq(seq)
    }
}

struct Features<'a> {
    sink: &'a mut Sink,
}

impl<'de> DeserializeSeed<'de> for Features<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Features<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an array of features")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while let Some(f) = seq.next_element::<Json>()? {
            self.sink.feature(f);
        }
        Ok(())
    }
}
