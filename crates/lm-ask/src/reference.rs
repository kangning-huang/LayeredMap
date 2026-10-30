//! Built-in reference layers (owner's design, 2026-10-18): Natural Earth 1:10m countries,
//! states and provinces, cities, rivers, lakes, coastline, land, minor islands, regions and seas,
//! and NOAA ETOPO 2022 elevation, inside NimbleMap Pro. They never show on the map; the assistant
//! sees them in the map context and a plan may name them like any layer ("count Power plants in
//! each World country"), which loads them then. Country borders follow the system region: the
//! Natural Earth point of view for that region (China's on a Mac set to mainland China), applied
//! from the manifest's per-view changes. Built by `tools/lm/reference/build_reference.py`; the
//! data and its sources are documented in that folder's README.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value as Json;

use lm_core::document::{DocLayer, Style};
use lm_core::model::LayerId;
use lm_jobs::import::{Ctx, LayerData, import_path};
use lm_jobs::plan_exec::PlanLayer;

/// The names the assistant and the user see, by manifest key.
const NAMES: &[(&str, &str)] = &[
    ("countries", "World countries"),
    ("states_provinces", "World states and provinces"),
    ("cities", "World cities"),
    ("rivers", "World rivers"),
    ("lakes", "World lakes"),
    ("coastline", "World coastline"),
    ("land", "World land"),
    ("minor_islands", "World minor islands"),
    ("regions", "World regions"),
    ("seas", "World seas"),
    ("elevation", "World elevation"),
];

/// Layer ids for built-in layers in plans (never document ids).
const ID_BASE: u64 = 20_000;

/// One built-in layer as the manifest describes it.
#[derive(Clone, Debug)]
pub struct RefLayer {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub count: u64,
    /// (field, type, meaning, unit)
    pub fields: Vec<(String, String, String, Option<String>)>,
}

pub struct Reference {
    dir: PathBuf,
    manifest: Json,
    /// The point of view applied to countries (`chn`, `usa`…), when the region has one.
    pub pov: Option<String>,
    loaded: Mutex<HashMap<String, PlanLayer>>,
}

/// Where the data is: `LM_REFERENCE_DIR`, the bundle's `Contents/Resources/reference` (on Windows
/// `Resources\reference` beside the exe), or the builder's copy on the LaCie drive (development).
pub fn default_dir() -> Option<PathBuf> {
    let ok = |p: PathBuf| p.join("manifest.json").is_file().then_some(p);
    if let Some(p) = std::env::var_os("LM_REFERENCE_DIR").map(PathBuf::from).and_then(ok) {
        return Some(p);
    }
    if let Some(p) = lm_core::platform::current_resources_dir().map(|r| r.join("reference")).and_then(ok) {
        return Some(p);
    }
    ok(PathBuf::from("/Volumes/LaCie/data/layeredmap-reference"))
}

/// The system region as an ISO 3166-1 alpha-2 code ("CN" from a locale "en_CN" or "zh-Hans-CN").
pub fn region_of_locale(locale: &str) -> Option<String> {
    let last = locale.split(['_', '-', '@', '.']).rfind(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()))?;
    Some(last.to_string())
}

/// This Mac's region (System Settings › General › Language & Region), from the locale APIs
/// (`sys_locale`), not `defaults read`.
#[cfg(target_os = "macos")]
pub fn system_region() -> Option<String> {
    sys_locale::get_locale().as_deref().and_then(region_of_locale)
}

/// This PC's region: Settings › Time & language › Language & region › Country or region (the
/// registry's `Geo\Name`, an ISO code since Windows 10), else the regional format's locale
/// ("zh-CN"). The country, not the format, is what decides the borders.
#[cfg(windows)]
pub fn system_region() -> Option<String> {
    let intl = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Control Panel\International").ok()?;
    let geo: Option<String> = intl.open_subkey("Geo").ok().and_then(|g| g.get_value("Name").ok());
    region_of_windows(geo.as_deref(), intl.get_value::<String, _>("LocaleName").ok().as_deref())
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn system_region() -> Option<String> {
    std::env::var("LANG").ok().and_then(|l| region_of_locale(&l))
}

/// The Windows region from the registry's `Geo\Name` (a two-letter country, or a three-digit UN
/// M.49 area such as "001" for World, which has no point of view) and `LocaleName`.
pub fn region_of_windows(geo_name: Option<&str>, locale_name: Option<&str>) -> Option<String> {
    let geo = geo_name.map(str::trim).filter(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_alphabetic()));
    geo.map(str::to_ascii_uppercase).or_else(|| locale_name.and_then(region_of_locale))
}

impl Reference {
    /// Opens the data in `dir` for a system `region` (its point of view, if Natural Earth has one).
    pub fn open(dir: &Path, region: Option<&str>) -> Option<Reference> {
        let manifest: Json = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).ok()?).ok()?;
        let pov = region.and_then(|r| manifest["pov"]["region_to_pov"][r].as_str().map(str::to_string));
        Some(Reference { dir: dir.to_path_buf(), manifest, pov, loaded: Mutex::new(HashMap::new()) })
    }

    /// The built-in layers, in a fixed order.
    pub fn catalogue(&self) -> Vec<RefLayer> {
        let mut out = Vec::new();
        for (key, name) in NAMES {
            let m = &self.manifest["layers"][*key];
            if m.is_null() {
                continue;
            }
            let fields = m["fields"]
                .as_object()
                .map(|fs| {
                    fs.iter()
                        .map(|(f, d)| {
                            (
                                f.clone(),
                                d["type"].as_str().unwrap_or("text").to_string(),
                                d["meaning"].as_str().unwrap_or("").to_string(),
                                d["unit"].as_str().map(str::to_string),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(RefLayer {
                key: key.to_string(),
                name: name.to_string(),
                kind: m["kind"].as_str().unwrap_or("areas").to_string(),
                description: m["description"].as_str().unwrap_or("").to_string(),
                count: m["features"].as_u64().unwrap_or(0),
                fields,
            });
        }
        out
    }

    /// The layer named `name` ("World countries"), loaded once and kept.
    pub fn layer(&self, name: &str) -> Option<PlanLayer> {
        let (i, r) = self.catalogue().into_iter().enumerate().find(|(_, r)| r.name.eq_ignore_ascii_case(name.trim()))?;
        if let Some(l) = self.loaded.lock().ok()?.get(&r.key) {
            return Some(l.clone());
        }
        let file = self.manifest["layers"][&r.key]["file"].as_str()?;
        let path = self.dir.join(file);
        let mut ls = import_path(&path, &Ctx::detached(Default::default())).ok()?;
        if ls.is_empty() {
            return None;
        }
        let l = ls.remove(0);
        let mut data = l.data;
        if r.key == "countries"
            && let (Some(pov), LayerData::Vector(ds)) = (&self.pov, &data)
        {
            match crate::pov::apply(&self.dir, &self.manifest, pov, ds) {
                Ok(v) => data = LayerData::Vector(Arc::new(v)),
                Err(e) => log::warn!("countries: point of view {pov} not applied: {e}"),
            }
        }
        let mut profile = l.profile.clone();
        if let LayerData::Vector(ds) = &data {
            profile.count = ds.features.len() as u64;
        }
        let style = Style::for_profile(&profile, [0, 0, 0, 255]);
        let pl = PlanLayer {
            name: r.name.clone(),
            alias: None,
            layer: DocLayer::new(LayerId(ID_BASE + i as u64), r.name.clone(), profile, style),
            data,
            rows: None,
            path: Some(path),
        };
        self.loaded.lock().ok()?.insert(r.key.clone(), pl.clone());
        Some(pl)
    }

    /// Lines for the map context, for the built-in layers whose names no open layer uses.
    pub fn context_lines(&self, first: usize, taken: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = first;
        for r in self.catalogue() {
            if taken.iter().any(|t| t.eq_ignore_ascii_case(&r.name)) {
                continue;
            }
            let mut s = format!("{i}. {}: {}", r.name, r.kind);
            if r.kind != "raster" {
                s.push_str(&format!(", {}", crate::context::thousands(r.count as usize)));
            }
            s.push_str(&format!(" ({})", r.description));
            if r.key == "countries"
                && let Some(p) = &self.pov
            {
                s.push_str(&format!("; borders as seen from the user's region ({p})"));
            }
            if !r.fields.is_empty() {
                let fs: Vec<String> = r
                    .fields
                    .iter()
                    .map(|(f, t, m, u)| match u {
                        Some(u) => format!("{f} ({t}, {u}: {m})"),
                        None => format!("{f} ({t}: {m})"),
                    })
                    .collect();
                s.push_str(&format!(". Fields: {}", fs.join("; ")));
            }
            out.push(s);
            i += 1;
        }
        out
    }
}

/// Every layer name a plan's arguments mention (layer arguments and objects' layers).
pub fn names_in(v: &Json, out: &mut Vec<String>) {
    match v {
        Json::String(s) => out.push(s.clone()),
        Json::Array(a) => a.iter().for_each(|x| names_in(x, out)),
        Json::Object(o) => o.values().for_each(|x| names_in(x, out)),
        _ => {}
    }
}
