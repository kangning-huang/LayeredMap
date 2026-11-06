//! A table whose location is one column: `POINT (-87.6 41.8)`, `(41.8, -87.6)`, `41.8, -87.6`,
//! or an address with the pair on its last line (`123 Main St\nChicago, IL\n(41.8, -87.6)`).
//!
//! In a survey of 10,000 open-data tables (Socrata catalogue, 2026-11), 2,039 held their
//! location this way — `location`, `geolocation`, `georeference`, `the_geom`, `point`,
//! `coordinates`, `ubicación`, `coordenadas`… — often with no latitude/longitude columns at all.
//! NASA's meteorite table has one beside its `reclat`/`reclong` (`GeoLocation`).

/// How a column writes each point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointOrder {
    /// WKT: `POINT (lon lat)`.
    Wkt,
    /// A bracketed or bare pair, latitude first: the way Socrata, Google and most people write it.
    LatLon,
    /// A pair written longitude first (seen when the first numbers run past 90).
    LonLat,
}

/// A column that holds each record's location as one value.
#[derive(Clone, Debug, PartialEq)]
pub struct PointColumn {
    pub index: usize,
    pub name: String,
    pub order: PointOrder,
}

/// Share of non-blank sample cells that must read as a point in range.
const SHARE: f64 = 0.95;

/// Words in a header that suggest it holds a location (a bare pair of numbers could be anything
/// else; WKT says what it is and needs no hint).
const HINTS: &[&str] = &[
    "location",
    "geolocation",
    "georeference",
    "georeferenced",
    "geocode",
    "geocoded",
    "point",
    "coordinates",
    "coordinate",
    "coords",
    "coord",
    "latlng",
    "latlon",
    "latlong",
    "lnglat",
    "lonlat",
    "position",
    "geo",
    "geometry",
    "geom",
    "the",
    "shape",
    "wkt",
    "place",
    "ubicacion",
    "ubicación",
    "coordenadas",
    "localizacion",
    "localización",
    "georreferenciada",
    "lat",
    "long",
    "lng",
    "lon",
    "latitude",
    "longitude",
];

fn hinted(header: &str) -> bool {
    let lower = header.to_lowercase();
    if ["坐标", "位置", "经纬"].iter().any(|w| lower.contains(w)) {
        return true;
    }
    crate::words(header).iter().any(|w| HINTS.contains(&w.as_str()))
}

fn number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// `POINT (x y)` (optionally `SRID=…;`, `Z`/`M`), as (lon, lat).
fn wkt_point(cell: &str) -> Option<(f64, f64)> {
    let mut s = cell.trim();
    if let Some((srid, rest)) = s.split_once(';')
        && srid.trim().to_ascii_uppercase().starts_with("SRID=")
    {
        s = rest.trim();
    }
    let upper = s.to_ascii_uppercase();
    let rest = upper.strip_prefix("POINT")?;
    let rest = rest.trim_start().trim_start_matches(['Z', 'M']).trim();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    let mut nums = inner.split_whitespace().map(number);
    let x = nums.next()??;
    let y = nums.next()??;
    Some((x, y))
}

/// The last `(a, b)` in the cell, else the whole cell as `a, b`: as written, first then second.
fn pair(cell: &str) -> Option<(f64, f64)> {
    let s = cell.trim();
    let inner = match (s.rfind('('), s.rfind(')')) {
        (Some(open), Some(close)) if open < close => &s[open + 1..close],
        _ => s,
    };
    let (a, b) = inner.split_once(',')?;
    Some((number(a)?, number(b)?))
}

/// One cell as (lon, lat), read as `order` says.
pub fn parse_point(cell: &str, order: PointOrder) -> Option<(f64, f64)> {
    let (lon, lat) = match order {
        PointOrder::Wkt => wkt_point(cell)?,
        PointOrder::LatLon => {
            let (a, b) = pair(cell)?;
            (b, a)
        }
        PointOrder::LonLat => pair(cell)?,
    };
    (lon.abs() <= 180.0 && lat.abs() <= 90.0).then_some((lon, lat))
}

/// The first column whose sample values are points in nearly every non-blank row.
pub fn point_column(headers: &[String], rows: &[Vec<String>]) -> Option<PointColumn> {
    for (index, header) in headers.iter().enumerate() {
        let cells: Vec<&str> = rows.iter().filter_map(|r| r.get(index)).map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
        if cells.is_empty() {
            continue;
        }
        let fits = |order| cells.iter().filter(|c| parse_point(c, order).is_some()).count() as f64 >= SHARE * cells.len() as f64;
        let found = |order| Some(PointColumn { index, name: header.trim().to_string(), order });
        if fits(PointOrder::Wkt) {
            return found(PointOrder::Wkt);
        }
        if !hinted(header) {
            continue;
        }
        // Latitude first, unless a first number runs past 90: then it is a longitude.
        let lon_first = cells.iter().filter_map(|c| pair(c)).any(|(a, b)| a.abs() > 90.0 && b.abs() <= 90.0);
        let order = if lon_first { PointOrder::LonLat } else { PointOrder::LatLon };
        if fits(order) {
            return found(order);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn wkt_points_need_no_hint() {
        let h = s(&["name", "anything"]);
        let r = vec![s(&["a", "POINT (-87.6 41.8)"]), s(&["b", "SRID=4326;POINT(-87.7 41.9)"]), s(&["c", ""])];
        let p = point_column(&h, &r).expect("found");
        assert_eq!((p.index, p.order), (1, PointOrder::Wkt));
        assert_eq!(parse_point("POINT (-87.6 41.8)", p.order), Some((-87.6, 41.8)));
    }

    #[test]
    fn socrata_pairs_and_addresses_read_latitude_first() {
        let h = s(&["name", "GeoLocation"]);
        let r = vec![s(&["Aachen", "(50.775, 6.08333)"]), s(&["X", "123 Main St\nChicago, IL\n(41.8, -87.6)"]), s(&["Y", "41.9, -87.7"])];
        let p = point_column(&h, &r).expect("found");
        assert_eq!(p.order, PointOrder::LatLon);
        assert_eq!(parse_point("(50.775, 6.08333)", p.order), Some((6.08333, 50.775)));
        assert_eq!(parse_point("123 Main St\nChicago, IL\n(41.8, -87.6)", p.order), Some((-87.6, 41.8)));
    }

    #[test]
    fn pairs_written_longitude_first() {
        let h = s(&["name", "coordinates"]);
        let r = vec![s(&["a", "121.47, 31.23"]), s(&["b", "-87.6, 41.8"])];
        let p = point_column(&h, &r).expect("found");
        assert_eq!(p.order, PointOrder::LonLat);
        assert_eq!(parse_point("121.47, 31.23", p.order), Some((121.47, 31.23)));
    }

    #[test]
    fn a_pair_without_a_location_name_is_not_a_location() {
        let h = s(&["name", "score range"]);
        let r = vec![s(&["a", "(1, 2)"]), s(&["b", "(3, 4)"])];
        assert_eq!(point_column(&h, &r), None);
    }
}
