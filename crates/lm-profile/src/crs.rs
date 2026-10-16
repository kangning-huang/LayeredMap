//! Catalog rows "No coordinate system; values within ±180 / ±90" and "No coordinate system;
//! projected-looking values".

use crate::{lines, make};
use lm_core::model::{Assumption, AssumptionKind, Choice, Confidence, Consequence, Crs, Extent, Known, Subject, Treatment};

/// True when every coordinate of `extent` is a plausible longitude/latitude (±180 / ±90) and
/// the extent is not empty.
pub fn looks_like_lonlat(extent: &Extent) -> bool {
    let finite = [extent.min_x, extent.min_y, extent.max_x, extent.max_y].iter().all(|v| v.is_finite());
    finite
        && extent.min_x <= extent.max_x
        && extent.min_y <= extent.max_y
        && extent.min_x >= -180.0
        && extent.max_x <= 180.0
        && extent.min_y >= -90.0
        && extent.max_y <= 90.0
}

/// The coordinate system of a layer that has geometry (a Shapefile, a GeoPackage table…), from
/// what the file declares or, failing that, from the range of its raw coordinates.
///
/// - `declared` given (a `.prj`, GeoTIFF keys, GeoJSON's RFC 7946 default): `Certain`, no
///   assumption.
/// - Nothing declared and `raw_extent` within ±180 / ±90: WGS 84, `Likely`, with a visible
///   [`AssumptionKind::NoCrsLonLat`].
/// - Nothing declared and anything else (projected-looking values such as 350,000 / 3,450,000,
///   or no coordinates at all): `Crs::Unknown`, with the [`AssumptionKind::NoCrsProjected`]
///   question at import ("Where should this data be?"). Its Not sure opens the file as a table.
///
/// CSV files go through [`crate::csv_locate`] instead, whose line already names WGS 84.
pub fn crs_from_values(raw_extent: Option<Extent>, declared: Option<Crs>) -> (Known<Crs>, Vec<Assumption>) {
    if let Some(crs) = declared.filter(|c| *c != Crs::Unknown) {
        return (Known::certain(crs, "Declared in the file."), vec![]);
    }
    match raw_extent {
        Some(e) if looks_like_lonlat(&e) => {
            let evidence = format!(
                "No coordinate system in the file; coordinates lie within ±180 / ±90 ({:.4} to {:.4}, {:.4} to {:.4}).",
                e.min_x, e.max_x, e.min_y, e.max_y
            );
            let a = make(
                AssumptionKind::NoCrsLonLat,
                Subject::Layer,
                Choice::new("wgs84", "GPS coordinates (WGS 84)"),
                vec![
                    Choice::new("wgs84", "GPS coordinates (WGS 84)"),
                    Choice::new("find_place", "Choose where it should be…"),
                    Choice::not_sure(),
                ],
                Confidence::Likely,
                Consequence::ChangesLocations,
                Treatment::Visible,
                lines::NO_CRS_LONLAT,
            );
            (Known::likely(Crs::Wgs84, evidence), vec![a])
        }
        other => {
            let evidence = match other {
                Some(e) => format!(
                    "No coordinate system in the file; coordinates are not degrees ({:.0} to {:.0}, {:.0} to {:.0}).",
                    e.min_x, e.max_x, e.min_y, e.max_y
                ),
                None => "No coordinate system in the file and no coordinates to judge from.".into(),
            };
            (Known::unknown(Crs::Unknown, evidence), vec![projected_question()])
        }
    }
}

/// The "Where should this data be?" question (also used by [`crate::csv_locate`]).
pub(crate) fn projected_question() -> Assumption {
    make(
        AssumptionKind::NoCrsProjected,
        Subject::Layer,
        Choice::new("find_place", "Show where it should be"),
        vec![Choice::new("find_place", "Show where it should be"), Choice::not_sure()],
        Confidence::Unknown,
        Consequence::ChangesLocations,
        Treatment::Question,
        lines::NO_CRS_PROJECTED,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(a: f64, b: f64, c: f64, d: f64) -> Extent {
        Extent { min_x: a, min_y: b, max_x: c, max_y: d }
    }

    #[test]
    fn no_crs_lonlat_matches_degree_values() {
        let (crs, a) = crs_from_values(Some(ext(121.25, 30.95, 121.75, 31.45)), None);
        assert_eq!(crs.value, Crs::Wgs84);
        assert_eq!(crs.confidence, Confidence::Likely);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].kind, AssumptionKind::NoCrsLonLat);
        assert_eq!(a[0].treatment, Treatment::Visible);
        assert_eq!(a[0].line, "Assumed GPS coordinates (WGS 84), because the file doesn't say.");
        assert_eq!(a[0].alternatives.last().map(|c| c.id.as_str()), Some("not_sure"));
    }

    #[test]
    fn no_crs_lonlat_does_not_match_declared_or_projected() {
        let (crs, a) = crs_from_values(Some(ext(121.25, 30.95, 121.75, 31.45)), Some(Crs::Epsg(4326)));
        assert_eq!(crs.confidence, Confidence::Certain);
        assert!(a.is_empty());
        let (_, a) = crs_from_values(Some(ext(614006.6, 3420504.5, 672100.3, 3487815.1)), None);
        assert!(a.iter().all(|a| a.kind != AssumptionKind::NoCrsLonLat));
    }

    #[test]
    fn no_crs_projected_matches_metre_values() {
        let (crs, a) = crs_from_values(Some(ext(350_000.0, 3_450_000.0, 360_000.0, 3_460_000.0)), None);
        assert_eq!(crs.value, Crs::Unknown);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].kind, AssumptionKind::NoCrsProjected);
        assert_eq!(a[0].treatment, Treatment::Question);
        assert_eq!(a[0].line, "Where should this data be?");
        assert!(a[0].alternatives.iter().any(|c| c.id == "not_sure"));
        // A latitude out of range is not degrees either.
        let (_, a) = crs_from_values(Some(ext(10.0, 10.0, 20.0, 95.0)), None);
        assert_eq!(a[0].kind, AssumptionKind::NoCrsProjected);
    }

    #[test]
    fn no_crs_projected_does_not_match_degrees_or_declared() {
        let (_, a) = crs_from_values(Some(ext(-10.0, -10.0, 10.0, 10.0)), None);
        assert!(a.iter().all(|a| a.kind != AssumptionKind::NoCrsProjected));
        let (_, a) = crs_from_values(Some(ext(350_000.0, 3_450_000.0, 360_000.0, 3_460_000.0)), Some(Crs::Epsg(4549)));
        assert!(a.is_empty());
    }
}
