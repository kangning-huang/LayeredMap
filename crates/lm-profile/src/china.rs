//! Catalog rows "Points inside mainland China, assumed WGS 84" and "As above, plus the name hints
//! at the source".

use crate::china_boundary::CHINA_PARTS;
use crate::{lines, make};
use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, Crs, Known, Subject, Treatment};

/// Share of the given points that must lie in mainland China.
const INSIDE_SHARE: f64 = 0.9;

/// True when (`lon`, `lat`) lies in mainland China: inside a simplified Natural Earth boundary
/// (the mainland and Hainan; Taiwan, Hong Kong and Macau are separate units there and not
/// included, though the 0.1° outward buffer reaches into the edge of Hong Kong).
pub fn in_mainland_china(lon: f64, lat: f64) -> bool {
    if !(73.0..=136.0).contains(&lon) || !(17.5..=54.0).contains(&lat) {
        return false;
    }
    CHINA_PARTS.iter().any(|ring| in_ring(ring, lon, lat))
}

fn in_ring(ring: &[(f32, f32)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let (Some(&(xi, yi)), Some(&(xj, yj))) = (ring.get(i), ring.get((i + n - 1) % n)) else { continue };
        let (xi, yi, xj, yj) = (f64::from(xi), f64::from(yi), f64::from(xj), f64::from(yj));
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
    }
    inside
}

/// Share of `points` (longitude, latitude) in mainland China; 0 for no points.
pub fn mainland_china_share(points: &[(f64, f64)]) -> f64 {
    let finite: Vec<&(f64, f64)> = points.iter().filter(|(x, y)| x.is_finite() && y.is_finite()).collect();
    if finite.is_empty() {
        return 0.0;
    }
    finite.iter().filter(|(x, y)| in_mainland_china(*x, *y)).count() as f64 / finite.len() as f64
}

/// The offset system a name hints at, from the catalog's list of hints.
fn name_hint(names: &[&str]) -> Option<Crs> {
    // Checked in order: the file name first, then the columns; within a name, Baidu's hints
    // are more specific ("bd09" also appears next to "gcj" in conversion tools' output).
    for n in names {
        let l = n.to_lowercase();
        let compact: String = l.chars().filter(|c| c.is_alphanumeric()).collect();
        if compact.contains("baidu") || l.contains("百度") || compact.contains("bd09") {
            return Some(Crs::Bd09);
        }
        if compact.contains("gcj") || compact.contains("amap") || compact.contains("gaode") || l.contains("高德") {
            return Some(Crs::Gcj02);
        }
    }
    None
}

/// The China offset assumption for a layer whose positions are *assumed* WGS 84.
///
/// - `points`: longitude/latitude of the features (a sample; for lines and areas, vertices or
///   centroids).
/// - `crs`: the layer's coordinate system as profiled. Only an assumed WGS 84 (not `Certain`)
///   can be an offset; a declared one is taken at its word.
/// - `file_name`, `column_names`: searched for the catalog's hints (gcj, amap, gaode, 高德 →
///   GCJ-02; baidu, 百度, bd09 → BD-09).
///
/// When at least 90% of the points lie in mainland China:
/// - with a hint → [`AssumptionKind::ChinaOffsetNamed`], a question at import with the
///   conversion preselected;
/// - without → [`AssumptionKind::ChinaOffsetPossible`], visible with Compare; points stay.
///
/// Both offer Not sure (points stay, the notice stays until dismissed or converted).
pub fn china_offset(points: &[(f64, f64)], crs: &Known<Crs>, file_name: &str, column_names: &[String]) -> Option<Assumption> {
    if crs.value != Crs::Wgs84 || crs.confidence == Confidence::Certain {
        return None;
    }
    if mainland_china_share(points) < INSIDE_SHARE {
        return None;
    }
    let wgs = Choice::new("wgs84", "GPS coordinates (WGS 84): keep where they are");
    let gcj = Choice::new("gcj02", "Amap/Tencent coordinates (GCJ-02): convert");
    let bd = Choice::new("bd09", "Baidu coordinates (BD-09): convert");
    let mut names: Vec<&str> = vec![file_name];
    names.extend(column_names.iter().map(String::as_str));
    Some(match name_hint(&names) {
        Some(Crs::Bd09) => make(
            AssumptionKind::ChinaOffsetNamed,
            Subject::Layer,
            bd.clone(),
            vec![bd, wgs, gcj, Choice::not_sure()],
            Confidence::Likely,
            Consequence::ChangesLocations,
            Treatment::Question,
            lines::CHINA_OFFSET_BD09,
        ),
        Some(_) => make(
            AssumptionKind::ChinaOffsetNamed,
            Subject::Layer,
            gcj.clone(),
            vec![gcj, wgs, bd, Choice::not_sure()],
            Confidence::Likely,
            Consequence::ChangesLocations,
            Treatment::Question,
            lines::CHINA_OFFSET_GCJ02,
        ),
        None => make(
            AssumptionKind::ChinaOffsetPossible,
            Subject::Layer,
            wgs.clone(),
            vec![wgs, gcj, bd, Choice::not_sure()],
            Confidence::Possible,
            Consequence::ChangesLocations,
            Treatment::Visible,
            lines::CHINA_OFFSET_POSSIBLE,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assumed() -> Known<Crs> {
        Known::likely(Crs::Wgs84, "lon/lat columns")
    }
    fn shanghai() -> Vec<(f64, f64)> {
        (0..10).map(|i| (121.3 + i as f64 * 0.04, 31.0 + i as f64 * 0.04)).collect()
    }

    #[test]
    fn boundary_knows_cities() {
        for (name, lon, lat) in [
            ("Beijing", 116.40, 39.90),
            ("Shanghai", 121.47, 31.23),
            ("Shanghai coast (Pudong airport)", 121.80, 31.14),
            ("Urumqi", 87.62, 43.83),
            ("Lhasa", 91.11, 29.65),
            ("Harbin", 126.53, 45.80),
            ("Haikou", 110.32, 20.04),
            ("Kunming", 102.83, 24.88),
        ] {
            assert!(in_mainland_china(lon, lat), "{name}");
        }
        for (name, lon, lat) in [
            ("Taipei", 121.56, 25.04),
            ("Seoul", 126.98, 37.57),
            ("Tokyo", 139.69, 35.69),
            ("Hanoi", 105.85, 21.03),
            ("Ulaanbaatar", 106.91, 47.92),
            ("New Delhi", 77.21, 28.61),
            ("Manila", 120.98, 14.60),
            ("New York", -74.0, 40.7),
        ] {
            assert!(!in_mainland_china(lon, lat), "{name}");
        }
    }

    #[test]
    fn china_offset_possible_matches_points_in_china() {
        let a = china_offset(&shanghai(), &assumed(), "schools.csv", &["lon".into(), "lat".into()]);
        let a = a.unwrap();
        assert_eq!(a.kind, AssumptionKind::ChinaOffsetPossible);
        assert_eq!(a.treatment, Treatment::Visible);
        assert_eq!(a.chosen.id, "wgs84");
        assert_eq!(a.line, "If these came from a Chinese map app (Amap, Tencent or Baidu), they may sit a few hundred meters off.");
        assert_eq!(a.alternatives.last(), Some(&Choice::not_sure()));
    }

    #[test]
    fn china_offset_possible_does_not_match_outside_china_or_declared() {
        let nyc: Vec<(f64, f64)> = (0..10).map(|i| (-74.0 + i as f64 * 0.01, 40.7)).collect();
        assert_eq!(china_offset(&nyc, &assumed(), "amap.csv", &[]), None);
        assert_eq!(china_offset(&shanghai(), &Known::certain(Crs::Wgs84, ".prj"), "x.shp", &[]), None);
        assert_eq!(china_offset(&shanghai(), &Known::likely(Crs::Epsg(4549), "?"), "x.shp", &[]), None);
        assert_eq!(china_offset(&[], &assumed(), "x.csv", &[]), None);
        // Mostly outside.
        let mut mixed = nyc.clone();
        mixed.push((121.4, 31.2));
        assert_eq!(china_offset(&mixed, &assumed(), "x.csv", &[]), None);
    }

    #[test]
    fn china_offset_named_matches_hints() {
        for (file, cols, want, line) in [
            ("amap_pois.csv", vec!["lng", "lat"], "gcj02", lines::CHINA_OFFSET_GCJ02),
            ("pois.csv", vec!["Lng_GCJ", "Lat_GCJ"], "gcj02", lines::CHINA_OFFSET_GCJ02),
            ("高德导出.csv", vec!["x", "y"], "gcj02", lines::CHINA_OFFSET_GCJ02),
            ("Gaode.csv", vec![], "gcj02", lines::CHINA_OFFSET_GCJ02),
            ("baidu_pois.csv", vec!["lng", "lat"], "bd09", lines::CHINA_OFFSET_BD09),
            ("百度.csv", vec![], "bd09", lines::CHINA_OFFSET_BD09),
            ("p.csv", vec!["lng_bd09", "lat_bd09"], "bd09", lines::CHINA_OFFSET_BD09),
            ("p.csv", vec!["BD-09 lng", "BD-09 lat"], "bd09", lines::CHINA_OFFSET_BD09),
        ] {
            let cols: Vec<String> = cols.into_iter().map(String::from).collect();
            let a = china_offset(&shanghai(), &assumed(), file, &cols).unwrap();
            assert_eq!(a.kind, AssumptionKind::ChinaOffsetNamed, "{file}");
            assert_eq!(a.treatment, Treatment::Question);
            assert_eq!(a.chosen.id, want, "{file}");
            assert_eq!(a.line, line);
            assert!(a.alternatives.iter().any(|c| c.id == "wgs84"));
            assert_eq!(a.alternatives.last(), Some(&Choice::not_sure()));
        }
    }

    #[test]
    fn china_offset_named_does_not_match_without_hints() {
        let a = china_offset(&shanghai(), &assumed(), "pois.csv", &["lng".into(), "lat".into()]).unwrap();
        assert_ne!(a.kind, AssumptionKind::ChinaOffsetNamed);
        // A hint outside China is no offset.
        let paris = vec![(2.35, 48.85)];
        assert_eq!(china_offset(&paris, &assumed(), "amap.csv", &[]), None);
    }
}
