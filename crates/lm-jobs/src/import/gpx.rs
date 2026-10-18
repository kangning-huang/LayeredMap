//! GPX (1.0 and 1.1): waypoints become points, tracks and routes lines, each kind its own layer.
//! A track is one feature (its segments are the parts of one line), as a trip is one thing to
//! the person who recorded it. A waypoint's elevation and time are fields; a track or route has
//! its first and last time (`start_time`, `end_time`) and lowest and highest elevation
//! (`min_ele`, `max_ele`) instead, besides its name, description and the other text fields.

use std::path::Path;
use std::sync::Arc;

use lm_core::model::{Crs, Exclusion};
use rustgis_engine::data::geometry::Geometry;
use rustgis_engine::data::{Dataset, GeometryType};
use rustgis_engine::geom::Coord;

use super::limits::XML_MAX_BYTES;
use super::xmlfile::{self, Columns, child_text, is};
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, status_of, vector_profile};

pub struct Gpx;

/// Text fields of a waypoint, route or track kept as written, in this order.
const TEXT_FIELDS: &[&str] = &["name", "cmt", "desc", "src", "sym", "type", "number"];

impl FormatAdapter for Gpx {
    fn name(&self) -> &'static str {
        "GPX"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        match extension(path).as_str() {
            "gpx" => true,
            "xml" => String::from_utf8_lossy(head).contains("<gpx"),
            _ => false,
        }
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > XML_MAX_BYTES {
            return Err(xmlfile::too_large(&file_name(path), "GPX"));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let bytes = xmlfile::read_capped(path, "GPX", ctx)?;
        let (text, encoding_notice) = xmlfile::decode(&bytes);
        drop(bytes);
        let doc = xmlfile::parse(&text, &name, "GPX")?;
        ctx.ticket.check()?;
        let root = doc.root_element();
        if !is(&root, "gpx") {
            return Err(ImportError::Failed {
                message: format!("{name} isn't a GPX file (it has no <gpx> element). Your file has not been changed."),
                details: format!("root element <{}>", root.tag_name().name()),
            });
        }
        ctx.stage(Stage::Extent)?;
        let mut kinds = [
            Kind::new("waypoints", GeometryType::Point),
            Kind::new("tracks", GeometryType::Polyline),
            Kind::new("routes", GeometryType::Polyline),
        ];
        for n in root.children().filter(|c| c.is_element()) {
            match n.tag_name().name() {
                "wpt" => kinds[0].waypoint(&n),
                "trk" => kinds[1].line(&n, "trkseg", "trkpt"),
                "rte" => kinds[2].line(&n, "", "rtept"),
                _ => {}
            }
        }
        drop(doc);
        ctx.ticket.check()?;
        ctx.stage(Stage::Profile)?;
        // A kind with nothing that can be drawn gives no layer; its records are named in a notice.
        let mut file_notices: Vec<String> = encoding_notice.into_iter().collect();
        for k in kinds.iter().filter(|k| !k.geoms.is_empty() && k.geoms.iter().all(Option::is_none)) {
            let reasons: Vec<&str> = k.reasons.iter().flatten().map(String::as_str).collect();
            let mut distinct = reasons.clone();
            distinct.dedup();
            file_notices.push(format!(
                "{} {} couldn't be drawn and {} left out ({}).",
                k.geoms.len(),
                if k.geoms.len() == 1 { k.what.trim_end_matches('s') } else { k.what },
                if k.geoms.len() == 1 { "was" } else { "were" },
                distinct.join("; ").to_lowercase()
            ));
        }
        let present: Vec<Kind> = kinds.into_iter().filter(|k| k.geoms.iter().any(Option::is_some)).collect();
        if present.is_empty() {
            let message = if file_notices.iter().any(|n| n.contains("couldn't be drawn")) {
                format!("{name} holds no waypoints, tracks or routes with a usable position. Your file has not been changed.")
            } else {
                format!("{name} holds no waypoints, tracks or routes, so there is nothing to show. Your file has not been changed.")
            };
            return Err(ImportError::Unsupported(message));
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
        let multi = present.len() > 1;
        let mut out = Vec::new();
        for (i, k) in present.into_iter().enumerate() {
            let layer_name = if multi { format!("{stem} ({})", k.what) } else { stem.clone() };
            let (fields, rows) = k.cols.finish();
            let mut ds = Dataset::new(layer_name.clone(), k.geometry_type, fields);
            ds.source = Some(path.display().to_string());
            let mut exclusions = Vec::new();
            for (row, ((g, reason), attrs)) in k.geoms.into_iter().zip(k.reasons).zip(rows).enumerate() {
                if let Some(reason) = reason {
                    exclusions.push(Exclusion { row: row as u64 + 1, reason });
                }
                ds.push_next(g, attrs);
            }
            let mut notices: Vec<String> = if i == 0 { file_notices.clone() } else { Vec::new() };
            if k.dropped > 0 {
                let (n, verb) = if k.dropped == 1 {
                    ("1 point".to_string(), "was")
                } else {
                    (format!("{} points", lm_core::model::group_thousands(k.dropped as u64)), "were")
                };
                notices.push(format!("{n} without a usable position {verb} left out of their {}.", k.what));
            }
            let field_names: Vec<String> = ds.fields.iter().map(|f| f.name.clone()).collect();
            let member = multi.then(|| layer_name.clone());
            let mut profile =
                vector_profile(path, member, "GPX", &ds, Some(Crs::Wgs84), Vec::new(), exclusions, None, notices, &field_names);
            profile.crs.evidence = lm_core::model::Evidence("GPX coordinates are GPS coordinates (WGS 84) by definition.".into());
            out.push(ImportedLayer {
                name: layer_name,
                status: status_of(&profile),
                profile,
                data: LayerData::Vector(Arc::new(ds)),
                raw: None,
                style: None,
            });
        }
        Ok(out)
    }
}

/// The records of one kind as they are read.
struct Kind {
    what: &'static str,
    geometry_type: GeometryType,
    cols: Columns,
    geoms: Vec<Option<Geometry>>,
    reasons: Vec<Option<String>>,
    /// Track and route points without a usable position, left out of their lines.
    dropped: usize,
}

/// A point's position from its `lat` and `lon` attributes.
fn position(n: &roxmltree::Node) -> Result<Coord, &'static str> {
    let get = |a: &str| n.attribute(a).map(|v| v.trim().parse::<f64>());
    match (get("lon"), get("lat")) {
        (Some(Ok(x)), Some(Ok(y))) if x.is_finite() && y.is_finite() => {
            if x.abs() > 180.0 || y.abs() > 90.0 {
                Err("Coordinates outside longitude ±180 and latitude ±90")
            } else {
                Ok(Coord::new(x, y))
            }
        }
        (None, _) | (_, None) => Err("No position (lat and lon)"),
        _ => Err("Coordinates that aren't numbers"),
    }
}

impl Kind {
    fn new(what: &'static str, geometry_type: GeometryType) -> Kind {
        Kind { what, geometry_type, cols: Columns::default(), geoms: Vec::new(), reasons: Vec::new(), dropped: 0 }
    }

    fn text_fields(&mut self, n: &roxmltree::Node) {
        for key in TEXT_FIELDS {
            if let Some(v) = child_text(n, key) {
                self.cols.set(key, v);
            }
        }
    }

    fn waypoint(&mut self, n: &roxmltree::Node) {
        self.cols.start();
        self.text_fields(n);
        for key in ["ele", "time"] {
            if let Some(v) = child_text(n, key) {
                self.cols.set(key, v);
            }
        }
        match position(n) {
            Ok(c) => {
                self.geoms.push(Some(Geometry::Point(c)));
                self.reasons.push(None);
            }
            Err(e) => {
                self.geoms.push(None);
                self.reasons.push(Some(e.to_string()));
            }
        }
    }

    /// A track (`segment` = "trkseg") or a route (no segments), as one line of one part per
    /// segment. Times and elevations are summarised over all its points.
    fn line(&mut self, n: &roxmltree::Node, segment: &str, point: &str) {
        self.cols.start();
        self.text_fields(n);
        let segments: Vec<roxmltree::Node> = if segment.is_empty() { vec![*n] } else { n.children().filter(|c| is(c, segment)).collect() };
        let mut parts: Vec<Vec<Coord>> = Vec::new();
        let (mut first_time, mut last_time) = (None, None);
        let (mut min_ele, mut max_ele) = (f64::INFINITY, f64::NEG_INFINITY);
        for s in segments {
            let mut part = Vec::new();
            for p in s.children().filter(|c| is(c, point)) {
                match position(&p) {
                    Ok(c) => part.push(c),
                    Err(_) => self.dropped += 1,
                }
                if let Some(t) = child_text(&p, "time") {
                    first_time.get_or_insert_with(|| t.clone());
                    last_time = Some(t);
                }
                if let Some(e) = child_text(&p, "ele").and_then(|e| e.parse::<f64>().ok()).filter(|e| e.is_finite()) {
                    min_ele = min_ele.min(e);
                    max_ele = max_ele.max(e);
                }
            }
            if part.len() >= 2 {
                parts.push(part);
            }
        }
        if let Some(t) = first_time {
            self.cols.set("start_time", t);
        }
        if let Some(t) = last_time {
            self.cols.set("end_time", t);
        }
        if min_ele.is_finite() {
            self.cols.set("min_ele", min_ele.to_string());
            self.cols.set("max_ele", max_ele.to_string());
        }
        if parts.is_empty() {
            self.geoms.push(None);
            self.reasons.push(Some("Fewer than two points with a position".into()));
        } else {
            self.geoms.push(Some(Geometry::Line(parts)));
            self.reasons.push(None);
        }
    }
}
