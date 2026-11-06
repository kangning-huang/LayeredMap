//! Shapes MapLibre will draw.
//!
//! MapLibre drops a whole GeoJSON source when one polygon ring is malformed: Geofabrik's Taiwan
//! extract (8,169 admin areas, two rings unclosed or under four points) drew nothing at all, while
//! the Mac drew it fine. So every shape is checked before it is handed over: rings are closed,
//! rings too short to be an area and lines too short to be a line are dropped, and a feature with
//! nothing drawable left keeps its row without a place on the map.

use rustgis_data::{Coord, Dataset, Geometry};

fn finite(c: &Coord) -> bool {
    c.x.is_finite() && c.y.is_finite()
}

/// A ring closed and long enough to be an area (four positions, the last repeating the first).
fn ring(mut r: Vec<Coord>) -> Option<Vec<Coord>> {
    r.retain(finite);
    r.dedup();
    if let (Some(first), Some(last)) = (r.first().copied(), r.last().copied())
        && first != last
    {
        r.push(first);
    }
    (r.len() >= 4).then_some(r)
}

fn drawable(g: Geometry) -> Option<Geometry> {
    match g {
        Geometry::Point(c) => finite(&c).then_some(Geometry::Point(c)),
        Geometry::MultiPoint(mut cs) => {
            cs.retain(finite);
            (!cs.is_empty()).then_some(Geometry::MultiPoint(cs))
        }
        Geometry::Line(parts) => {
            let parts: Vec<Vec<Coord>> = parts
                .into_iter()
                .map(|mut p| {
                    // Not de-duplicated: a tornado that touched down and lifted at one spot is a
                    // two-point line on one point, and it is still a tornado (10,494 of them).
                    p.retain(finite);
                    p
                })
                .filter(|p| p.len() >= 2)
                .collect();
            (!parts.is_empty()).then_some(Geometry::Line(parts))
        }
        Geometry::Polygon(polys) => {
            let polys: Vec<Vec<Vec<Coord>>> = polys
                .into_iter()
                .filter_map(|rings| {
                    let mut rings = rings.into_iter();
                    let outer = ring(rings.next()?)?;
                    Some(std::iter::once(outer).chain(rings.filter_map(ring)).collect())
                })
                .collect();
            (!polys.is_empty()).then_some(Geometry::Polygon(polys))
        }
    }
}

/// Makes every shape drawable; returns how many features lost theirs entirely.
pub fn make_drawable(ds: &mut Dataset) -> usize {
    let mut lost = 0;
    for f in &mut ds.features {
        if let Some(g) = f.geometry.take() {
            f.geometry = drawable(g);
            if f.geometry.is_none() {
                lost += 1;
            }
        }
    }
    lost
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: f64, y: f64) -> Coord {
        Coord::new(x, y)
    }

    #[test]
    fn unclosed_rings_are_closed_and_short_ones_dropped() {
        let open = Geometry::Polygon(vec![vec![vec![c(0.0, 0.0), c(1.0, 0.0), c(1.0, 1.0)]]]);
        match drawable(open) {
            Some(Geometry::Polygon(p)) => assert_eq!(p[0][0].len(), 4, "closed"),
            other => panic!("{other:?}"),
        }
        assert_eq!(drawable(Geometry::Polygon(vec![vec![vec![c(30.0, 30.0), c(31.0, 31.0)]]])), None);
        // A bad hole goes, the area stays.
        let holed = Geometry::Polygon(vec![vec![vec![c(0.0, 0.0), c(5.0, 0.0), c(5.0, 5.0), c(0.0, 0.0)], vec![c(1.0, 1.0), c(2.0, 2.0)]]]);
        match drawable(holed) {
            Some(Geometry::Polygon(p)) => assert_eq!(p[0].len(), 1),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lines_and_points_that_cant_draw_go() {
        assert_eq!(drawable(Geometry::Line(vec![vec![c(1.0, 1.0)]])), None);
        assert_eq!(drawable(Geometry::Point(c(f64::NAN, 1.0))), None);
        assert!(drawable(Geometry::Line(vec![vec![c(1.0, 1.0), c(2.0, 2.0)]])).is_some());
    }
}
