//! KML, and KMZ (a zipped KML, checked against the archive limits first). Placemarks with
//! Point, LineString, LinearRing, Polygon (with holes), MultiGeometry and Google Earth tracks;
//! their name, description, ExtendedData (Data and SchemaData) and time stamps become fields.
//!
//! **Layers.** Each folder that holds places directly becomes a layer (places outside any folder
//! form one more, named after the file), and a folder mixing points, lines and areas is split as
//! a mixed GeoJSON is. A file with more than [`MAX_FOLDER_LAYERS`] such folders becomes one layer
//! per kind instead, with each place's folder in a `folder` field.
//!
//! **Styles.** Shared and inline `Style`s, and `StyleMap`s (their `normal` style), are resolved
//! for each place. A layer takes the colours of its most common style: icon colour for points,
//! line colour and width for lines, fill colour (its alpha as fill opacity), outline colour and
//! width for areas. KML colours are `aabbggrr`. When a layer's places differ in style, the style
//! of each is kept in a `style` field and a notice says so.
//!
//! NetworkLink, GroundOverlay, ScreenOverlay and PhotoOverlay are not opened; a notice says what
//! was left out.

use lm_core::brand::NAME as APP;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lm_core::document::Style;
use lm_core::model::{Crs, Exclusion, LayerKind};
use rustgis_engine::data::geometry::Geometry;
use rustgis_engine::data::{Dataset, GeometryType};
use rustgis_engine::geom::Coord;

use super::limits::{XML_MAX_BYTES, check_zip, zip_entries};
use super::xmlfile::{self, Columns, child, child_text, is, text_of};
use super::{Ctx, FormatAdapter, ImportError, ImportedLayer, LayerData, Metadata, Stage, extension, file_name, status_of, vector_profile};

pub struct Kml;
pub struct Kmz;

/// More folders with places than this, and the file opens as one layer per kind.
pub const MAX_FOLDER_LAYERS: usize = 20;

/// Folders nested deeper than this are read as part of their parent at this depth.
const MAX_DEPTH: usize = 64;

impl FormatAdapter for Kml {
    fn name(&self) -> &'static str {
        "KML"
    }

    fn sniff(&self, path: &Path, head: &[u8]) -> bool {
        match extension(path).as_str() {
            "kml" => true,
            "xml" => String::from_utf8_lossy(head).contains("<kml"),
            _ => false,
        }
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > XML_MAX_BYTES {
            return Err(xmlfile::too_large(&file_name(path), "KML"));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let bytes = xmlfile::read_capped(path, "KML", ctx)?;
        read_kml(path, &bytes, "KML", Vec::new(), ctx)
    }
}

impl FormatAdapter for Kmz {
    fn name(&self) -> &'static str {
        "KMZ"
    }

    fn sniff(&self, path: &Path, _head: &[u8]) -> bool {
        extension(path) == "kmz"
    }

    fn open_metadata(&self, path: &Path, _ctx: &Ctx) -> Result<Metadata, ImportError> {
        let m = std::fs::metadata(path).map_err(|e| ImportError::read(&file_name(path), e))?;
        if m.len() > super::limits::ZIP_MAX_EXPANDED {
            return Err(ImportError::Refused(format!(
                "{} is larger than 2 GB, the most {APP} reads as KMZ. Your file has not been changed.",
                file_name(path)
            )));
        }
        Ok(Metadata { size: m.len(), ..Metadata::default() })
    }

    fn stream_features(&self, path: &Path, _meta: &Metadata, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
        let name = file_name(path);
        let bytes = std::fs::read(path).map_err(|e| ImportError::read(&name, e))?;
        // The limits are checked from the central directory before anything is expanded.
        let entries = zip_entries(&bytes)?;
        check_zip(&entries)?;
        ctx.ticket.check()?;
        let is_fork = |n: &str| n.split('/').any(|p| p == "__MACOSX" || p.starts_with("._"));
        let kmls: Vec<&super::limits::ZipEntry> =
            entries.iter().filter(|e| !is_fork(&e.name) && e.name.to_ascii_lowercase().ends_with(".kml")).collect();
        // The main document: doc.kml at the top if there is one, otherwise the first KML listed.
        let Some(main) = kmls.iter().find(|e| e.name.eq_ignore_ascii_case("doc.kml")).or_else(|| kmls.first()).copied() else {
            return Err(ImportError::Unsupported(format!("{name} holds no KML document. Your file has not been changed.")));
        };
        if main.expanded > XML_MAX_BYTES {
            return Err(xmlfile::too_large(&name, "KML"));
        }
        let wanted = main.name.clone();
        // Expanded within the KML limit, whatever the directory claims.
        let budget = usize::try_from(XML_MAX_BYTES).unwrap_or(usize::MAX);
        let files =
            rustgis_engine::data::zip::read_matching_within(&bytes, |n| n == wanted, budget).map_err(|e| ImportError::read(&name, e))?;
        drop(bytes);
        let Some((_, kml)) = files.into_iter().next() else {
            return Err(ImportError::read(&name, format!("{} could not be expanded", main.name)));
        };
        let mut notices = Vec::new();
        if kmls.len() > 1 {
            notices.push(format!(
                "This KMZ holds {} KML documents; {APP} opened the main one, {}.",
                kmls.len(),
                main.name.rsplit('/').next().unwrap_or(&main.name)
            ));
        }
        read_kml(path, &kml, "KMZ", notices, ctx)
    }
}

/// A style as far as NimbleMap can show it; each part is set only when the file sets it.
#[derive(Clone, Debug, Default, PartialEq)]
struct KmlStyle {
    line: Option<[u8; 4]>,
    width: Option<f32>,
    poly: Option<[u8; 4]>,
    fill: Option<bool>,
    outline: Option<bool>,
    icon: Option<[u8; 4]>,
}

impl KmlStyle {
    fn from_node(n: &roxmltree::Node) -> KmlStyle {
        let mut s = KmlStyle::default();
        if let Some(l) = child(n, "LineStyle") {
            s.line = child_text(&l, "color").and_then(|c| kml_color(&c));
            s.width = child_text(&l, "width").and_then(|w| w.parse::<f32>().ok()).filter(|w| w.is_finite() && *w >= 0.0);
        }
        if let Some(p) = child(n, "PolyStyle") {
            s.poly = child_text(&p, "color").and_then(|c| kml_color(&c));
            s.fill = child_text(&p, "fill").and_then(|v| kml_bool(&v));
            s.outline = child_text(&p, "outline").and_then(|v| kml_bool(&v));
        }
        if let Some(i) = child(n, "IconStyle") {
            s.icon = child_text(&i, "color").and_then(|c| kml_color(&c));
        }
        s
    }

    /// Sets what `o` sets.
    fn merge(&mut self, o: &KmlStyle) {
        self.line = o.line.or(self.line);
        self.width = o.width.or(self.width);
        self.poly = o.poly.or(self.poly);
        self.fill = o.fill.or(self.fill);
        self.outline = o.outline.or(self.outline);
        self.icon = o.icon.or(self.icon);
    }

    fn key(&self) -> String {
        format!("{self:?}")
    }

    /// "line #E41A1C, fill #377EB8": for the `style` field when the style has no name.
    fn describe(&self) -> String {
        let hex = |c: [u8; 4]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
        let parts: Vec<String> = [("icon", self.icon), ("line", self.line), ("fill", self.poly)]
            .into_iter()
            .filter_map(|(what, c)| c.map(|c| format!("{what} {}", hex(c))))
            .collect();
        if parts.is_empty() { "default".into() } else { parts.join(", ") }
    }
}

/// A KML colour, `aabbggrr` (alpha, blue, green, red), as red–green–blue–alpha. Six digits are
/// read as `bbggrr`, opaque.
fn kml_color(s: &str) -> Option<[u8; 4]> {
    let h = s.trim().trim_start_matches('#');
    let byte = |i: usize| h.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    match h.len() {
        8 => Some([byte(6)?, byte(4)?, byte(2)?, byte(0)?]),
        6 => Some([byte(4)?, byte(2)?, byte(0)?, 0xFF]),
        _ => None,
    }
}

fn kml_bool(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// The document's named styles and style maps.
#[derive(Default)]
struct Styles {
    shared: HashMap<String, KmlStyle>,
    /// A StyleMap's `normal` pair: another style's URL, or a style written inside it.
    maps: HashMap<String, (Option<String>, Option<KmlStyle>)>,
}

impl Styles {
    fn read(doc: &roxmltree::Document) -> Styles {
        let mut s = Styles::default();
        for n in doc.descendants().filter(|n| n.is_element()) {
            let Some(id) = n.attribute("id") else { continue };
            if is(&n, "Style") {
                s.shared.entry(id.to_string()).or_insert_with(|| KmlStyle::from_node(&n));
            } else if is(&n, "StyleMap") {
                let normal = n.children().filter(|p| is(p, "Pair")).find(|p| child_text(p, "key").is_some_and(|k| k == "normal"));
                if let Some(p) = normal {
                    let url = child_text(&p, "styleUrl");
                    let inline = child(&p, "Style").map(|st| KmlStyle::from_node(&st));
                    s.maps.entry(id.to_string()).or_insert((url, inline));
                }
            }
        }
        s
    }

    /// The style a `styleUrl` names (only its `#fragment` is looked up: styles in other files
    /// aren't fetched).
    fn resolve(&self, url: &str, depth: u8) -> Option<KmlStyle> {
        let id = url.rsplit_once('#').map_or(url, |(_, f)| f).trim();
        if depth > 4 || id.is_empty() {
            return None;
        }
        if let Some(s) = self.shared.get(id) {
            return Some(s.clone());
        }
        let (url, inline) = self.maps.get(id)?;
        let mut s = url.as_deref().and_then(|u| self.resolve(u, depth + 1)).unwrap_or_default();
        if let Some(i) = inline {
            s.merge(i);
        }
        Some(s)
    }
}

/// One place as read: its folder, fields, shapes (or why it has none) and style.
struct Place {
    folder: Vec<String>,
    attrs: Vec<(String, String)>,
    shapes: Result<Vec<Geometry>, String>,
    /// The `style` field's value and the resolved style.
    style: Option<(String, KmlStyle)>,
}

/// What the walk through the document found besides places.
#[derive(Default)]
struct Skipped {
    network_links: usize,
    ground_overlays: usize,
    screen_overlays: usize,
    /// Containers nested too deeply to be read.
    too_deep: usize,
}

fn walk(n: roxmltree::Node, folder: &mut Vec<String>, depth: usize, styles: &Styles, places: &mut Vec<Place>, skipped: &mut Skipped) {
    if depth > 4 * MAX_DEPTH {
        skipped.too_deep += 1;
        return;
    }
    for c in n.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "Folder" => {
                let named = depth < MAX_DEPTH;
                if named {
                    folder.push(child_text(&c, "name").unwrap_or_else(|| "Folder".to_string()));
                }
                walk(c, folder, depth + 1, styles, places, skipped);
                if named {
                    folder.pop();
                }
            }
            "Document" => walk(c, folder, depth + 1, styles, places, skipped),
            "Placemark" => places.push(place(&c, folder, styles)),
            "NetworkLink" => skipped.network_links += 1,
            "GroundOverlay" | "PhotoOverlay" => skipped.ground_overlays += 1,
            "ScreenOverlay" => skipped.screen_overlays += 1,
            _ => {}
        }
    }
}

fn place(n: &roxmltree::Node, folder: &[String], styles: &Styles) -> Place {
    let mut attrs = Vec::new();
    for key in ["name", "description"] {
        if let Some(v) = child_text(n, key) {
            attrs.push((key.to_string(), v));
        }
    }
    if let Some(ext) = child(n, "ExtendedData") {
        for d in ext.children().filter(|c| c.is_element()) {
            if is(&d, "Data") {
                if let (Some(k), Some(v)) = (d.attribute("name"), child_text(&d, "value")) {
                    attrs.push((k.to_string(), v));
                }
            } else if is(&d, "SchemaData") {
                for s in d.children().filter(|s| is(s, "SimpleData")) {
                    if let (Some(k), Some(v)) = (s.attribute("name"), text_of(&s)) {
                        attrs.push((k.to_string(), v));
                    }
                }
            }
        }
    }
    if let Some(t) = child(n, "TimeStamp").and_then(|t| child_text(&t, "when")) {
        attrs.push(("timestamp".into(), t));
    }
    if let Some(span) = child(n, "TimeSpan") {
        for key in ["begin", "end"] {
            if let Some(v) = child_text(&span, key) {
                attrs.push((key.into(), v));
            }
        }
    }
    let url = child_text(n, "styleUrl");
    let inline = child(n, "Style").map(|s| KmlStyle::from_node(&s));
    let style = match (&url, inline) {
        (None, None) => None,
        (url, inline) => {
            let mut s = url.as_deref().and_then(|u| styles.resolve(u, 0)).unwrap_or_default();
            let own = inline.is_some();
            if let Some(i) = &inline {
                s.merge(i);
            }
            let label = match url.as_deref().map(|u| u.rsplit_once('#').map_or(u, |(_, f)| f).trim()) {
                Some(id) if !id.is_empty() && !own => id.to_string(),
                _ => s.describe(),
            };
            Some((label, s))
        }
    };
    let geometry = n.children().find(|c| {
        c.is_element()
            && matches!(
                c.tag_name().name(),
                "Point" | "LineString" | "LinearRing" | "Polygon" | "MultiGeometry" | "Track" | "MultiTrack" | "Model"
            )
    });
    let shapes = match geometry {
        None => Err("No shape".to_string()),
        Some(g) => shapes(&g, 0).and_then(combine).map_err(str::to_string),
    };
    Place { folder: folder.to_vec(), attrs, shapes, style }
}

/// Single-part shapes of a geometry element (a MultiGeometry gives all of its parts).
fn shapes(g: &roxmltree::Node, depth: usize) -> Result<Vec<Geometry>, &'static str> {
    if depth > MAX_DEPTH {
        return Err("A shape nested too deeply");
    }
    match g.tag_name().name() {
        "Point" => {
            let c = coords(g)?;
            c.first().map(|p| vec![Geometry::Point(*p)]).ok_or("A point without coordinates")
        }
        "LineString" | "LinearRing" => {
            let c = coords(g)?;
            if c.len() < 2 {
                return Err("A line with fewer than two points");
            }
            Ok(vec![Geometry::Line(vec![c])])
        }
        "Polygon" => {
            let ring = |b: roxmltree::Node| -> Result<Vec<Coord>, &'static str> {
                let r = child(&b, "LinearRing").ok_or("An area boundary without a ring")?;
                let mut c = coords(&r)?;
                close_ring(&mut c);
                if c.len() < 4 { Err("An area with fewer than three corners") } else { Ok(c) }
            };
            let outer = child(g, "outerBoundaryIs").ok_or("An area without an outer boundary")?;
            let mut rings = vec![ring(outer)?];
            for inner in g.children().filter(|c| is(c, "innerBoundaryIs")) {
                // Usually one ring per innerBoundaryIs; some writers put several in one.
                for r in inner.children().filter(|c| is(c, "LinearRing")) {
                    let mut c = coords(&r)?;
                    close_ring(&mut c);
                    if c.len() < 4 {
                        return Err("A hole with fewer than three corners");
                    }
                    rings.push(c);
                }
            }
            Ok(vec![Geometry::Polygon(vec![rings])])
        }
        "MultiGeometry" => {
            let mut out = Vec::new();
            for c in g.children().filter(|c| c.is_element()) {
                if matches!(
                    c.tag_name().name(),
                    "Point" | "LineString" | "LinearRing" | "Polygon" | "MultiGeometry" | "Track" | "MultiTrack"
                ) {
                    out.extend(shapes(&c, depth + 1)?);
                }
            }
            if out.is_empty() { Err("An empty shape") } else { Ok(out) }
        }
        "Track" => {
            // gx:Track: `gx:coord` (or KML 2.3 `coord`) holds "lon lat [alt]".
            let mut pts = Vec::new();
            for c in g.children().filter(|c| is(c, "coord")) {
                let t = text_of(&c).unwrap_or_default();
                let mut it = t.split_whitespace().map(str::parse::<f64>);
                match (it.next(), it.next()) {
                    (Some(Ok(x)), Some(Ok(y))) => pts.push(lonlat(x, y)?),
                    _ => return Err("Coordinates that aren't numbers"),
                }
            }
            if pts.len() < 2 { Err("A track with fewer than two points") } else { Ok(vec![Geometry::Line(vec![pts])]) }
        }
        "MultiTrack" => {
            let mut out = Vec::new();
            for t in g.children().filter(|c| is(c, "Track")) {
                out.extend(shapes(&t, depth + 1)?);
            }
            if out.is_empty() { Err("An empty shape") } else { Ok(out) }
        }
        // These are `&'static str`, so the name is a literal: keep it in step with lm_core::brand::NAME.
        "Model" => Err("A 3D model, which NimbleMap doesn't show"),
        _ => Err("A shape NimbleMap can't read"),
    }
}

/// Closes a ring whose last point isn't its first.
fn close_ring(c: &mut Vec<Coord>) {
    if let Some(first) = c.first().copied()
        && c.last() != Some(&first)
    {
        c.push(first);
    }
}

fn lonlat(x: f64, y: f64) -> Result<Coord, &'static str> {
    if !x.is_finite() || !y.is_finite() {
        return Err("Coordinates that aren't numbers");
    }
    if x.abs() > 180.0 || y.abs() > 90.0 {
        return Err("Coordinates outside longitude ±180 and latitude ±90");
    }
    Ok(Coord::new(x, y))
}

/// The `coordinates` of an element: tuples "lon,lat[,alt]" separated by white space (the height
/// is dropped). A tuple split after its comma ("lon, lat") is joined again.
fn coords(n: &roxmltree::Node) -> Result<Vec<Coord>, &'static str> {
    let text = child(n, "coordinates").as_ref().and_then(text_of).unwrap_or_default();
    let mut tuples: Vec<String> = Vec::new();
    let mut joining = false;
    for t in text.split_whitespace() {
        match tuples.last_mut() {
            Some(last) if joining || t.starts_with(',') => last.push_str(t),
            _ => tuples.push(t.to_string()),
        }
        joining = t.ends_with(',');
    }
    tuples
        .iter()
        .map(|t| {
            let mut it = t.split(',').map(|v| v.trim().parse::<f64>());
            match (it.next(), it.next()) {
                (Some(Ok(x)), Some(Ok(y))) => lonlat(x, y),
                _ => Err("Coordinates that aren't numbers"),
            }
        })
        .collect()
}

/// Parts of one kind become one shape (several points a MultiPoint; lines one multi-part line;
/// areas one multi-part area). A place mixing kinds gives one shape per kind.
fn combine(parts: Vec<Geometry>) -> Result<Vec<Geometry>, &'static str> {
    let (mut points, mut lines, mut polys) = (Vec::new(), Vec::new(), Vec::new());
    for p in parts {
        match p {
            Geometry::Point(c) => points.push(c),
            Geometry::MultiPoint(v) => points.extend(v),
            Geometry::Line(l) => lines.extend(l),
            Geometry::Polygon(p) => polys.extend(p),
        }
    }
    let mut out = Vec::new();
    match points.len() {
        0 => {}
        1 => out.push(Geometry::Point(points[0])),
        _ => out.push(Geometry::MultiPoint(points)),
    }
    if !lines.is_empty() {
        out.push(Geometry::Line(lines));
    }
    if !polys.is_empty() {
        out.push(Geometry::Polygon(polys));
    }
    if out.is_empty() { Err("An empty shape") } else { Ok(out) }
}

fn kind_of_geom(g: &Geometry) -> LayerKind {
    match g {
        Geometry::Point(_) | Geometry::MultiPoint(_) => LayerKind::Points,
        Geometry::Line(_) => LayerKind::Lines,
        Geometry::Polygon(_) => LayerKind::Areas,
    }
}

/// The layer style for a KML style, where it sets colours for this kind of layer; `None` keeps
/// the palette colour. Icons draw as circles in the icon's colour (a white icon has none).
fn layer_style(kind: LayerKind, count: u64, s: &KmlStyle) -> Option<Style> {
    let rgb = |c: [u8; 4]| [c[0], c[1], c[2], 0xFF];
    let alpha = |c: [u8; 4]| f32::from(c[3]) / 255.0;
    match kind {
        LayerKind::Points => {
            let c = s.icon.filter(|c| c[..3] != [0xFF, 0xFF, 0xFF])?;
            Some(Style { opacity: alpha(c), ..Style::for_kind(kind, rgb(c), count) })
        }
        LayerKind::Lines => {
            let c = s.line?;
            let mut st = Style { opacity: alpha(c), ..Style::for_kind(kind, rgb(c), count) };
            if let Some(w) = s.width {
                st.line_width = w.clamp(0.5, 12.0);
            }
            Some(st)
        }
        LayerKind::Areas => {
            let fill = s.poly.or(s.line)?;
            let mut st = Style::for_kind(kind, rgb(fill), count);
            if s.poly.is_some() {
                st.fill_opacity = alpha(fill);
            }
            if s.fill == Some(false) {
                st.fill_opacity = 0.0;
            }
            if let Some(l) = s.line {
                st.stroke = rgb(l);
            }
            if let Some(w) = s.width {
                st.stroke_width = w.clamp(0.0, 12.0);
            }
            if s.outline == Some(false) {
                st.stroke_width = 0.0;
            }
            Some(st)
        }
        _ => None,
    }
}

/// One layer before profiling.
struct Part {
    name: String,
    ds: Dataset,
    exclusions: Vec<Exclusion>,
    style: Option<Style>,
    notices: Vec<String>,
    split: Option<lm_core::model::Assumption>,
}

/// Reads a KML document's bytes into layers.
fn read_kml(path: &Path, bytes: &[u8], format: &str, mut notices: Vec<String>, ctx: &Ctx) -> Result<Vec<ImportedLayer>, ImportError> {
    let name = file_name(path);
    let (text, encoding_notice) = xmlfile::decode(bytes);
    notices.extend(encoding_notice);
    let doc = xmlfile::parse(&text, &name, "KML")?;
    ctx.ticket.check()?;
    let root = doc.root_element();
    if !is(&root, "kml") {
        return Err(ImportError::Failed {
            message: format!("{name} isn't a KML file (it has no <kml> element). Your file has not been changed."),
            details: format!("root element <{}>", root.tag_name().name()),
        });
    }
    let styles = Styles::read(&doc);
    let mut places = Vec::new();
    let mut skipped = Skipped::default();
    walk(root, &mut Vec::new(), 0, &styles, &mut places, &mut skipped);
    drop(doc);
    ctx.ticket.check()?;
    let mut left_out = Vec::new();
    let plural = |n: usize, one: &str, many: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {many}") };
    if skipped.network_links > 0 {
        left_out.push(plural(skipped.network_links, "link to another file (a network link)", "links to other files (network links)"));
    }
    if skipped.ground_overlays > 0 {
        left_out.push(plural(
            skipped.ground_overlays,
            "picture laid over the map (a ground overlay)",
            "pictures laid over the map (ground overlays)",
        ));
    }
    if skipped.screen_overlays > 0 {
        left_out.push(plural(
            skipped.screen_overlays,
            "picture fixed on the screen (a screen overlay)",
            "pictures fixed on the screen (screen overlays)",
        ));
    }
    if skipped.too_deep > 0 {
        left_out.push(plural(skipped.too_deep, "folder nested too deeply to read", "folders nested too deeply to read"));
    }
    if places.is_empty() {
        let message = if left_out.is_empty() {
            format!("{name} holds no places (Placemarks), so there is nothing to show. Your file has not been changed.")
        } else {
            format!(
                "{name} holds no places {APP} can show, only {}, which it doesn't open. Your file has not been changed.",
                left_out.join(" and ")
            )
        };
        return Err(ImportError::Unsupported(message));
    }
    if !left_out.is_empty() {
        notices.push(format!("This file also has {}, which {APP} doesn't open.", left_out.join(" and ")));
    }
    ctx.stage(Stage::Extent)?;
    // Folders → layers, unless there are too many.
    let mut folders: Vec<Vec<String>> = Vec::new();
    for p in &places {
        if !folders.contains(&p.folder) {
            folders.push(p.folder.clone());
        }
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("layer").to_string();
    let combined = folders.len() > MAX_FOLDER_LAYERS;
    if combined {
        notices.push(format!(
            "This file has {} folders, more than {APP} shows as separate layers; each place's folder is in the field folder.",
            folders.len()
        ));
        for p in &mut places {
            let f = p.folder.join(" / ");
            p.attrs.push(("folder".into(), f));
            p.folder.clear();
        }
        folders = vec![Vec::new()];
    }
    let label = |f: &Vec<String>| -> String {
        let Some(last) = f.last() else { return stem.clone() };
        let clashes = folders.iter().filter(|g| g.last() == Some(last)).count() > 1;
        if clashes { f.join(" / ") } else { last.clone() }
    };
    let mut parts: Vec<Part> = Vec::new();
    for f in &folders {
        ctx.ticket.check()?;
        let group: Vec<&Place> = places.iter().filter(|p| &p.folder == f).collect();
        parts.extend(group_parts(path, &label(f), &group));
    }
    ctx.stage(Stage::Profile)?;
    // Every layer of a file needs its own name, so each can be found again.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for p in &mut parts {
        let n = seen.entry(p.name.to_lowercase()).or_insert(0);
        *n += 1;
        if *n > 1 {
            p.name = format!("{} ({n})", p.name);
        }
    }
    let multi = parts.len() > 1;
    let mut out = Vec::new();
    for (i, p) in parts.into_iter().enumerate() {
        let mut layer_notices = if i == 0 { notices.clone() } else { Vec::new() };
        layer_notices.extend(p.notices);
        let field_names: Vec<String> = p.ds.fields.iter().map(|f| f.name.clone()).collect();
        let member = multi.then(|| p.name.clone());
        let mut profile = vector_profile(
            path,
            member,
            format,
            &p.ds,
            Some(Crs::Wgs84),
            p.split.into_iter().collect(),
            p.exclusions,
            None,
            layer_notices,
            &field_names,
        );
        profile.crs.evidence = lm_core::model::Evidence("KML coordinates are GPS coordinates (WGS 84) by definition.".into());
        out.push(ImportedLayer {
            name: p.name,
            status: status_of(&profile),
            profile,
            data: LayerData::Vector(Arc::new(p.ds)),
            raw: None,
            style: p.style,
        });
    }
    Ok(out)
}

/// The layers of one folder: its places as a dataset, split by kind, each part with its
/// exclusions and the style most of its places have.
fn group_parts(path: &Path, name: &str, group: &[&Place]) -> Vec<Part> {
    // Whether places of one kind differ in style: then the style is kept per place.
    let mut keys: HashMap<LayerKind, Vec<String>> = HashMap::new();
    for p in group {
        if let (Ok(shapes), Some((_, s))) = (&p.shapes, &p.style) {
            for g in shapes {
                let k = keys.entry(kind_of_geom(g)).or_default();
                let key = s.key();
                if !k.contains(&key) {
                    k.push(key);
                }
            }
        }
    }
    let mixed_styles = keys.values().any(|k| k.len() > 1);
    let mut cols = Columns::default();
    let mut geoms: Vec<Option<Geometry>> = Vec::new();
    let mut reasons: Vec<Option<String>> = Vec::new();
    let mut styles: Vec<Option<&KmlStyle>> = Vec::new();
    let mut add = |p: &Place, g: Option<Geometry>, reason: Option<String>| {
        cols.start();
        for (k, v) in &p.attrs {
            cols.set(k, v.clone());
        }
        if mixed_styles && let Some((label, _)) = &p.style {
            cols.set("style", label.clone());
        }
        geoms.push(g);
        reasons.push(reason);
    };
    for p in group {
        match &p.shapes {
            Ok(shapes) => {
                for g in shapes {
                    add(p, Some(g.clone()), None);
                    styles.push(p.style.as_ref().map(|(_, s)| s));
                }
            }
            Err(reason) => {
                add(p, None, Some(reason.clone()));
                styles.push(None);
            }
        }
    }
    let (fields, rows) = cols.finish();
    let mut ds = Dataset::new(name, GeometryType::None, fields);
    ds.source = Some(path.display().to_string());
    for (g, row) in geoms.into_iter().zip(rows) {
        ds.push_next(g, row);
    }
    // Feature ids are 1, 2, 3… in the order pushed: they find each feature's reason and style
    // after the split.
    let at = |id: u64| usize::try_from(id).ok().and_then(|i| i.checked_sub(1));
    let parts = super::split_by_kind(ds);
    let kinds: Vec<LayerKind> = parts.iter().map(|(k, _)| *k).collect();
    let split = lm_profile::mixed_geometry_split(&kinds);
    parts
        .into_iter()
        .map(|(kind, ds)| {
            let exclusions = ds
                .features
                .iter()
                .enumerate()
                .filter(|(_, f)| f.geometry.is_none())
                .map(|(i, f)| Exclusion {
                    row: i as u64 + 1,
                    reason: at(f.id).and_then(|j| reasons.get(j).cloned().flatten()).unwrap_or_else(|| "No shape".into()),
                })
                .collect();
            let mut counts: Vec<(String, &KmlStyle, usize)> = Vec::new();
            for f in ds.features.iter().filter(|f| f.geometry.is_some()) {
                if let Some(s) = at(f.id).and_then(|j| styles.get(j).copied().flatten()) {
                    let key = s.key();
                    match counts.iter_mut().find(|(k, _, _)| *k == key) {
                        Some(c) => c.2 += 1,
                        None => counts.push((key, s, 1)),
                    }
                }
            }
            let count = super::located_count(&ds, kind);
            // The most common style; the first seen wins a tie.
            let best = counts.iter().enumerate().max_by_key(|(i, c)| (c.2, std::cmp::Reverse(*i))).map(|(_, c)| c.1);
            let style = best.and_then(|s| layer_style(kind, count, s));
            let mut notices = Vec::new();
            if counts.len() > 1 {
                notices.push(format!(
                    "The places in this layer have {} different styles in the file; the layer is drawn in the most common one, and each place's style is in the field style.",
                    counts.len()
                ));
            }
            Part { name: ds.name.clone(), ds, exclusions, style, notices, split: split.clone() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kml_colours_are_alpha_blue_green_red() {
        assert_eq!(kml_color("7f0000ff"), Some([0xFF, 0x00, 0x00, 0x7F]));
        assert_eq!(kml_color("#ff00ff00"), Some([0x00, 0xFF, 0x00, 0xFF]));
        assert_eq!(kml_color("ff0000"), Some([0x00, 0x00, 0xFF, 0xFF]));
        assert_eq!(kml_color("red"), None);
        assert_eq!(kml_color("ff00zz00"), None);
    }

    #[test]
    fn styles_become_layer_styles() {
        let s = KmlStyle { line: Some([255, 0, 0, 255]), width: Some(4.0), poly: Some([0, 0, 255, 128]), ..KmlStyle::default() };
        let a = layer_style(LayerKind::Areas, 3, &s).unwrap();
        assert_eq!(a.fill, [0, 0, 255, 255]);
        assert!((a.fill_opacity - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(a.stroke, [255, 0, 0, 255]);
        assert_eq!(a.stroke_width, 4.0);
        let l = layer_style(LayerKind::Lines, 3, &s).unwrap();
        assert_eq!((l.stroke, l.line_width, l.opacity), ([255, 0, 0, 255], 4.0, 1.0));
        // A white icon has no colour of its own; a tinted one does.
        assert!(layer_style(LayerKind::Points, 3, &KmlStyle { icon: Some([255, 255, 255, 255]), ..KmlStyle::default() }).is_none());
        let p = layer_style(LayerKind::Points, 3, &KmlStyle { icon: Some([0, 128, 0, 255]), ..KmlStyle::default() }).unwrap();
        assert_eq!(p.fill, [0, 128, 0, 255]);
        let hollow = KmlStyle { fill: Some(false), outline: Some(true), ..s };
        assert_eq!(layer_style(LayerKind::Areas, 3, &hollow).unwrap().fill_opacity, 0.0);
    }

    #[test]
    fn coordinates_tolerate_spaces_after_commas() {
        let doc = roxmltree::Document::parse("<LineString><coordinates>1,2,0 3, 4  5,6\n</coordinates></LineString>").unwrap();
        let c = coords(&doc.root_element()).unwrap();
        assert_eq!(c, vec![Coord::new(1.0, 2.0), Coord::new(3.0, 4.0), Coord::new(5.0, 6.0)]);
        let doc = roxmltree::Document::parse("<Point><coordinates>200,10</coordinates></Point>").unwrap();
        assert!(coords(&doc.root_element()).is_err());
    }
}
