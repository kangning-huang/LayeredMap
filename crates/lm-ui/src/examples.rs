//! The example data behind the Examples menu (WP2.9; owner's request 2026-10-18 for a list to
//! choose from): world basics, plus datasets a map app usually can't show (storm tracks, walled
//! cities, big earthquakes, volcanoes, airports, power plants). Sources and licences:
//! `assets/examples/README.md`.

use std::path::{Path, PathBuf};

use lm_core::tn;

/// One entry of the Examples menu: its files open together as one batch.
pub struct Example {
    pub id: &'static str,
    /// English, the catalog key: translated where it is drawn (`tl!(e.title)`), like `blurb`.
    pub title: &'static str,
    /// One line under the title.
    pub blurb: &'static str,
    /// A Phosphor icon that hints at the subject, and its tint (owner's request, 2026-10-18).
    pub icon: &'static str,
    pub tint: [u8; 3],
    pub files: &'static [&'static str],
}

pub const EXAMPLES: &[Example] = &[
    Example {
        id: "world",
        icon: egui_phosphor::regular::GLOBE_HEMISPHERE_EAST,
        tint: [0, 114, 178],
        title: tn!("World basics"),
        blurb: tn!("Cities, countries and elevation"),
        files: &["cities.geojson", "countries.geojson", "elevation.tif"],
    },
    Example {
        id: "heatwaves",
        icon: egui_phosphor::regular::THERMOMETER_HOT,
        tint: [213, 60, 30],
        title: tn!("Heatwaves, summer 2023"),
        blurb: tn!("Daily heatwave intensity worldwide, June to August"),
        files: &["heatwaves summer 2023.nc"],
    },
    Example {
        id: "storms",
        icon: egui_phosphor::regular::HURRICANE,
        tint: [86, 180, 233],
        title: tn!("Typhoons and hurricanes"),
        blurb: tn!("4,999 storm tracks since 1980"),
        files: &["storm tracks.gpkg"],
    },
    Example {
        id: "walls",
        icon: egui_phosphor::regular::CASTLE_TURRET,
        tint: [166, 118, 29],
        title: tn!("Walled cities of China, 1866"),
        blurb: tn!("1,789 city walls of late imperial China"),
        files: &["walled cities 1866.geojson"],
    },
    Example {
        id: "earthquakes",
        icon: egui_phosphor::regular::PULSE,
        tint: [213, 94, 0],
        title: tn!("Big earthquakes"),
        blurb: tn!("14,537 of magnitude 6 or more since 1900"),
        files: &["earthquakes.csv"],
    },
    Example {
        id: "volcanoes",
        icon: egui_phosphor::regular::MOUNTAINS,
        tint: [204, 51, 17],
        title: tn!("Volcanoes"),
        blurb: tn!("1,609 volcanoes and their status"),
        files: &["volcanoes.csv"],
    },
    Example {
        id: "airports",
        icon: egui_phosphor::regular::AIRPLANE_TILT,
        tint: [0, 158, 115],
        title: tn!("Airports"),
        blurb: tn!("5,280 large and medium airports, with elevation"),
        files: &["airports.csv"],
    },
    Example {
        id: "power",
        icon: egui_phosphor::regular::LIGHTNING,
        tint: [230, 159, 0],
        title: tn!("Power plants"),
        blurb: tn!("34,936 plants by fuel and capacity"),
        files: &["power plants.csv"],
    },
];

/// Each example file's licence, as `assets/examples/README.md` records it (shown in the map
/// package dialog, WP6.5). They are written into map packages, so they stay English.
pub const LICENSES: &[(&str, &str)] = &[
    ("cities.geojson", "Public domain (Natural Earth)"),
    ("countries.geojson", "Public domain (Natural Earth)"),
    ("elevation.tif", "Public domain, CC0 (NOAA ETOPO 2022)"),
    ("heatwaves summer 2023.nc", "CC BY 4.0, © European Union (Copernicus GDO)"),
    ("storm tracks.gpkg", "NOAA/WMO open access (IBTrACS); cite Knapp et al. 2010"),
    ("walled cities 1866.geojson", "CC BY 4.0 (Xue et al. 2021, ESSD)"),
    ("earthquakes.csv", "Public domain (U.S. Geological Survey)"),
    ("volcanoes.csv", "US Government work (NOAA NCEI)"),
    ("airports.csv", "Public domain (OurAirports)"),
    ("power plants.csv", "CC BY 4.0 (WRI Global Power Plant Database)"),
];

/// The licence of `path` when it is one of the example files (in an `examples` folder: the app
/// bundle's or the checkout's).
pub fn license_of(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    if path.parent()?.file_name()?.to_str()? != "examples" {
        return None;
    }
    LICENSES.iter().find(|(f, _)| *f == name).map(|(_, l)| *l)
}

/// Every example file (the bundle must hold them all to be used).
fn all_files() -> impl Iterator<Item = &'static str> {
    EXAMPLES.iter().flat_map(|e| e.files.iter().copied())
}

/// Where the files are in a development checkout.
const DEV_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/examples");

/// The examples folder inside an app bundle, `<NimbleMap.app>/Contents/Resources/examples`, for
/// an executable at `<NimbleMap.app>/Contents/MacOS/layeredmap` (on Windows `Resources\examples`
/// beside `layeredmap.exe`).
fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    Some(lm_core::platform::resources_dir(exe, lm_core::platform::Os::CURRENT)?.join("examples"))
}

/// The examples folder: the bundle's when it holds the files, otherwise the checkout's.
fn resolve(exe: Option<&Path>, exists: impl Fn(&Path) -> bool) -> PathBuf {
    exe.and_then(bundle_dir).filter(|d| all_files().all(|f| exists(&d.join(f)))).unwrap_or_else(|| PathBuf::from(DEV_DIR))
}

/// The files of the example `id` ("world" when unknown).
pub fn paths_of(id: &str) -> Vec<PathBuf> {
    let exe = std::env::current_exe().ok();
    let dir = resolve(exe.as_deref(), |p| p.is_file());
    let e = EXAMPLES.iter().find(|e| e.id == id).unwrap_or(&EXAMPLES[0]);
    e.files.iter().map(|f| dir.join(f)).collect()
}

/// The world basics (cities, countries, elevation).
pub fn paths() -> Vec<PathBuf> {
    paths_of("world")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_folder_is_next_to_the_executable() {
        let (exe, want) = if cfg!(windows) {
            (Path::new(r"C:\Program Files\NimbleMap\layeredmap.exe"), PathBuf::from(r"C:\Program Files\NimbleMap\Resources\examples"))
        } else {
            (
                Path::new("/Applications/NimbleMap.app/Contents/MacOS/layeredmap"),
                PathBuf::from("/Applications/NimbleMap.app/Contents/Resources/examples"),
            )
        };
        assert_eq!(bundle_dir(exe), Some(want.clone()));
        assert_eq!(resolve(Some(exe), |p| p.starts_with(&want)), want);
    }

    #[test]
    fn falls_back_to_the_checkout() {
        let exe = Path::new("/target/release/layeredmap");
        assert_eq!(resolve(Some(exe), |_| false), PathBuf::from(DEV_DIR));
        assert_eq!(resolve(None, |_| true), PathBuf::from(DEV_DIR));
        // A bundle missing one file is not used.
        let app = Path::new("/A.app/Contents/MacOS/layeredmap");
        assert_eq!(resolve(Some(app), |p| !p.ends_with("elevation.tif")), PathBuf::from(DEV_DIR));
    }

    #[test]
    fn every_example_file_has_a_licence() {
        for f in all_files() {
            assert!(license_of(&PathBuf::from(DEV_DIR).join(f)).is_some(), "{f}");
        }
        assert_eq!(license_of(Path::new("/Users/ana/Downloads/airports.csv")), None, "a user's file of the same name");
    }

    #[test]
    fn the_checkout_holds_every_file() {
        for e in EXAMPLES {
            for p in paths_of(e.id) {
                assert!(p.is_file(), "{} is missing", p.display());
            }
        }
    }

    /// The datasets beyond the world basics open with their kinds and sizes.
    #[test]
    fn every_menu_example_imports() {
        use lm_core::model::LayerKind;
        use lm_jobs::import::{Ctx, Ticket, import_path};
        for (id, kind, lo, hi) in [
            ("heatwaves", LayerKind::Raster, 0, u64::MAX),
            ("storms", LayerKind::Lines, 4_000, 6_000),
            ("walls", LayerKind::Areas, 1_500, 2_000),
            ("earthquakes", LayerKind::Points, 10_000, 20_000),
            ("volcanoes", LayerKind::Points, 1_000, 3_000),
            ("airports", LayerKind::Points, 4_000, 7_000),
            ("power", LayerKind::Points, 30_000, 40_000),
        ] {
            let p = paths_of(id).remove(0);
            let ls = import_path(&p, &Ctx::detached(Ticket::default())).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(ls[0].profile.kind, kind, "{id}");
            assert!((lo..hi).contains(&ls[0].profile.count), "{id}: {}", ls[0].profile.count);
        }
    }

    /// Each example opens through the import pipeline with the expected kind and a sensible size.
    #[test]
    fn every_example_imports() {
        use lm_core::model::LayerKind;
        use lm_jobs::import::{Ctx, ImportedLayer, LayerData, Ticket, import_path};
        let open = |name: &str| -> ImportedLayer {
            let p = PathBuf::from(DEV_DIR).join(name);
            let mut layers = match import_path(&p, &Ctx::detached(Ticket::default())) {
                Ok(l) => l,
                Err(e) => panic!("{name}: {e} ({})", e.details()),
            };
            assert_eq!(layers.len(), 1, "{name}");
            layers.remove(0)
        };
        let places = open("cities.geojson");
        assert_eq!(places.profile.kind, LayerKind::Points);
        assert!((1000..2000).contains(&places.profile.count), "{}", places.profile.count);
        let countries = open("countries.geojson");
        assert_eq!(countries.profile.kind, LayerKind::Areas);
        assert!((150..250).contains(&countries.profile.count), "{}", countries.profile.count);
        let elevation = open("elevation.tif");
        assert_eq!(elevation.profile.kind, LayerKind::Raster);
        let LayerData::Raster(r) = &elevation.data else { panic!("elevation is not a raster") };
        assert_eq!((r.width, r.height), (720, 360));
    }
}
