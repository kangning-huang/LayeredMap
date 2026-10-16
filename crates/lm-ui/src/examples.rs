//! The example data behind "Try an example" (WP2.9): Natural Earth places and countries, and ETOPO
//! 2022 elevation. Sources and licences: `assets/examples/README.md`.

use std::path::{Path, PathBuf};

/// The example files, opened together as one batch.
pub const FILES: [&str; 3] = ["populated_places.geojson", "countries.geojson", "elevation.tif"];

/// Where the files are in a development checkout.
const DEV_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/examples");

/// The examples folder inside an app bundle, `<LayeredMap.app>/Contents/Resources/examples`, for
/// an executable at `<LayeredMap.app>/Contents/MacOS/layeredmap`.
fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    Some(exe.parent()?.parent()?.join("Resources").join("examples"))
}

/// The examples folder: the bundle's when it holds the files, otherwise the checkout's.
fn resolve(exe: Option<&Path>, exists: impl Fn(&Path) -> bool) -> PathBuf {
    exe.and_then(bundle_dir).filter(|d| FILES.iter().all(|f| exists(&d.join(f)))).unwrap_or_else(|| PathBuf::from(DEV_DIR))
}

/// The three example files to open.
pub fn paths() -> Vec<PathBuf> {
    let exe = std::env::current_exe().ok();
    let dir = resolve(exe.as_deref(), |p| p.is_file());
    FILES.iter().map(|f| dir.join(f)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_folder_is_next_to_the_executable() {
        let exe = Path::new("/Applications/LayeredMap.app/Contents/MacOS/layeredmap");
        let want = PathBuf::from("/Applications/LayeredMap.app/Contents/Resources/examples");
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
    fn the_checkout_holds_every_file() {
        for p in paths() {
            assert!(p.is_file(), "{} is missing", p.display());
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
        let places = open("populated_places.geojson");
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
