//! Where a table's records are, when `rustgis-data`'s own exact-name rule (`lat`, `latitude`,
//! `y`, …) finds no coordinate columns.
//!
//! Both checks are the desktop's own, from `lm-profile`, so a table locates on the phone as it
//! does on the Mac:
//!
//! 1. [`lm_profile::csv_locate`]: coordinate columns by name — split into words and at case
//!    changes, glued names (`reclat`/`reclong`, `INTPTLAT`), Spanish/Italian spellings,
//!    pairs told apart by the rest of the name — with the values checked as degrees.
//! 2. [`lm_profile::point_column`]: the location as one column (`POINT (lon lat)`, `(lat, lon)`,
//!    an address ending in a pair), which a fifth of open-data tables use.

use rustgis_data::{Coord, Dataset, Geometry, GeometryType, Value};

/// Rows looked at.
const SAMPLE: usize = 2000;

pub enum Located {
    /// Read the file again from these x (longitude) and y (latitude) columns.
    Columns { x: String, y: String },
    /// The points were taken from one column; the table now holds them.
    OneColumn { name: String, skipped: usize },
    /// Coordinate columns whose values are not degrees (metres in a national grid, say).
    Projected { x: String, y: String },
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
    }
}

/// Finds the records' locations in a table read without them; for one column, fills them in.
pub fn locate(table: &mut Dataset, file_name: &str) -> Option<Located> {
    let headers: Vec<String> = table.fields.iter().map(|f| f.name.clone()).collect();
    let rows: Vec<Vec<String>> =
        table.features.iter().take(SAMPLE).map(|f| f.attrs.iter().map(cell).collect()).collect();

    match lm_profile::csv_locate(&headers, &rows, file_name) {
        lm_profile::CsvLocation::Located { pair, .. } => return Some(Located::Columns { x: pair.x, y: pair.y }),
        lm_profile::CsvLocation::Ambiguous { candidates, recommended, .. } => {
            if let Some(p) = candidates.get(recommended) {
                return Some(Located::Columns { x: p.x.clone(), y: p.y.clone() });
            }
        }
        lm_profile::CsvLocation::Projected { pair, .. } => {
            // A one-column location beats projected columns beside it.
            if lm_profile::point_column(&headers, &rows).is_none() {
                return Some(Located::Projected { x: pair.x, y: pair.y });
            }
        }
        lm_profile::CsvLocation::Table { .. } => {}
    }

    let col = lm_profile::point_column(&headers, &rows)?;
    let mut skipped = 0;
    table.features.retain_mut(|f| {
        let point = f.attrs.get(col.index).map(cell).and_then(|c| lm_profile::parse_point(&c, col.order));
        match point {
            Some((lon, lat)) => {
                f.geometry = Some(Geometry::Point(Coord::new(lon, lat)));
                true
            }
            None => {
                skipped += 1;
                false
            }
        }
    });
    if table.features.is_empty() {
        return None;
    }
    table.geometry_type = GeometryType::Point;
    Some(Located::OneColumn { name: col.name, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_meteorite_table() {
        let Ok(path) = std::env::var("LM_CSV") else { return };
        let (mut ds, _) = rustgis_data::io::read_path(std::path::Path::new(&path)).expect("read");
        match locate(&mut ds, "x.csv") {
            Some(Located::Columns { x, y }) => println!("columns x={x} y={y}"),
            Some(Located::OneColumn { name, skipped }) => println!("one column {name}, skipped {skipped}"),
            Some(Located::Projected { x, y }) => println!("projected {x} {y}"),
            None => println!("none"),
        }
    }
}
