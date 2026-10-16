//! "Where should this data be?" (WP2.4): instead of asking for an EPSG code, the user names a
//! place; every coordinate system whose area of use covers it is tried, and those that put the
//! data near the place are offered, nearest first.

use crate::crs_candidates::CANDIDATES;

/// Most candidates shown.
pub const MAX_CANDIDATES: usize = 6;
/// A candidate must put the data's centre within this distance of the place.
pub const MAX_DISTANCE_KM: f64 = 60.0;

/// A coordinate system that would put the data near the chosen place.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceCandidate {
    pub epsg: u16,
    pub name: String,
    /// Where the data's centre lands, longitude/latitude.
    pub lands: (f64, f64),
    /// How far that is from the place.
    pub distance_km: f64,
}

/// Great-circle distance in kilometres.
pub fn distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.1.to_radians(), b.1.to_radians());
    let dla = la2 - la1;
    let dlo = (b.0 - a.0).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * 6371.0088 * h.sqrt().asin()
}

/// The candidates for data whose raw (unknown-system) centre is `raw_center`, near `place`
/// (longitude, latitude). `to_lonlat(epsg, xy)` converts a point from a system (the caller has
/// the projection code). Nearest first, at most [`MAX_CANDIDATES`].
pub fn place_candidates(
    raw_center: (f64, f64),
    place: (f64, f64),
    to_lonlat: impl Fn(u16, (f64, f64)) -> Option<(f64, f64)>,
) -> Vec<PlaceCandidate> {
    let mut out: Vec<PlaceCandidate> = CANDIDATES
        .iter()
        .filter(|(_, _, [w, s, e, n])| place.0 >= w - 0.5 && place.0 <= e + 0.5 && place.1 >= s - 0.5 && place.1 <= n + 0.5)
        .filter_map(|(epsg, name, _)| {
            let lands = to_lonlat(*epsg, raw_center)?;
            if !(lands.0.is_finite() && lands.1.is_finite()) {
                return None;
            }
            let d = distance_km(lands, place);
            (d <= MAX_DISTANCE_KM).then(|| PlaceCandidate { epsg: *epsg, name: (*name).to_string(), lands, distance_km: d })
        })
        .collect();
    out.sort_by(|a, b| a.distance_km.total_cmp(&b.distance_km).then(a.epsg.cmp(&b.epsg)));
    out.truncate(MAX_CANDIDATES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_are_great_circle_kilometres() {
        let d = distance_km((121.47, 31.23), (116.40, 39.90));
        assert!((d - 1067.0).abs() < 10.0, "{d}");
    }

    #[test]
    fn only_systems_covering_the_place_and_landing_near_it_are_offered() {
        // A fake projection: CM 120E systems put the data on Shanghai, everything else far away.
        let shanghai = (121.47, 31.23);
        let to = |epsg: u16, _xy: (f64, f64)| if matches!(epsg, 4549 | 2385) { Some((121.46, 31.22)) } else { Some((0.0, 0.0)) };
        let c = place_candidates((350_000.0, 3_450_000.0), shanghai, to);
        let codes: Vec<u16> = c.iter().map(|x| x.epsg).collect();
        assert_eq!(codes, vec![2385, 4549]);
        assert!(c[0].distance_km < 2.0);
        // Nothing near London covers Shanghai's systems.
        assert!(place_candidates((0.0, 0.0), (-0.12, 51.5), to).is_empty());
    }

    #[test]
    fn the_table_covers_utm_and_chinese_zones() {
        for code in [32651u16, 32750, 4549, 4513, 2385, 2422, 27700] {
            assert!(CANDIDATES.iter().any(|(c, _, _)| *c == code), "EPSG:{code}");
        }
    }
}
