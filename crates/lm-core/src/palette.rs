//! Data palettes (Visual design system › Data palettes): sequential palettes for values, sampled
//! by position 0–1, and the "No data" colour. Categorical colours are [`crate::document::OKABE_ITO`].

use crate::document::Rgba;

/// Missing data: light gray (drawn with a hatch where the renderer can), labelled "No data".
pub const NO_DATA: Rgba = [0xD0, 0xD3, 0xD8, 0xFF];

/// Sequential palettes offered for values, in menu order.
pub const SEQUENTIAL: [&str; 4] = ["viridis", "cividis", "heat", "blues"];

const VIRIDIS: [Rgba; 9] = [
    [0x44, 0x01, 0x54, 0xFF],
    [0x47, 0x2D, 0x7B, 0xFF],
    [0x3B, 0x52, 0x8B, 0xFF],
    [0x2C, 0x72, 0x8E, 0xFF],
    [0x21, 0x91, 0x8C, 0xFF],
    [0x28, 0xAE, 0x80, 0xFF],
    [0x5E, 0xC9, 0x62, 0xFF],
    [0xAD, 0xDC, 0x30, 0xFF],
    [0xFD, 0xE7, 0x25, 0xFF],
];
const CIVIDIS: [Rgba; 9] = [
    [0x00, 0x22, 0x4E, 0xFF],
    [0x12, 0x35, 0x70, 0xFF],
    [0x3B, 0x49, 0x6C, 0xFF],
    [0x57, 0x5D, 0x6D, 0xFF],
    [0x70, 0x71, 0x73, 0xFF],
    [0x8A, 0x86, 0x78, 0xFF],
    [0xA5, 0x9C, 0x74, 0xFF],
    [0xC3, 0xB3, 0x69, 0xFF],
    [0xFE, 0xE8, 0x38, 0xFF],
];
/// ColorBrewer YlOrRd (9 classes).
const HEAT: [Rgba; 9] = [
    [0xFF, 0xFF, 0xCC, 0xFF],
    [0xFF, 0xED, 0xA0, 0xFF],
    [0xFE, 0xD9, 0x76, 0xFF],
    [0xFE, 0xB2, 0x4C, 0xFF],
    [0xFD, 0x8D, 0x3C, 0xFF],
    [0xFC, 0x4E, 0x2A, 0xFF],
    [0xE3, 0x1A, 0x1C, 0xFF],
    [0xBD, 0x00, 0x26, 0xFF],
    [0x80, 0x00, 0x26, 0xFF],
];
/// ColorBrewer Blues (9 classes).
const BLUES: [Rgba; 9] = [
    [0xF7, 0xFB, 0xFF, 0xFF],
    [0xDE, 0xEB, 0xF7, 0xFF],
    [0xC6, 0xDB, 0xEF, 0xFF],
    [0x9E, 0xCA, 0xE1, 0xFF],
    [0x6B, 0xAE, 0xD6, 0xFF],
    [0x42, 0x92, 0xC6, 0xFF],
    [0x21, 0x71, 0xB5, 0xFF],
    [0x08, 0x51, 0x9C, 0xFF],
    [0x08, 0x30, 0x6B, 0xFF],
];

fn stops(name: &str) -> &'static [Rgba; 9] {
    match name {
        "cividis" => &CIVIDIS,
        "heat" => &HEAT,
        "blues" => &BLUES,
        _ => &VIRIDIS,
    }
}

/// The palette's name in menus.
pub fn label(name: &str) -> &'static str {
    match name {
        "cividis" => "Cividis",
        "heat" => "Heat",
        "blues" => "Blues",
        _ => "Viridis",
    }
}

/// The colour at position `t` (0 = low, 1 = high), interpolated between stops.
pub fn sample(name: &str, t: f64) -> Rgba {
    let s = stops(name);
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let x = t * (s.len() - 1) as f64;
    let i = (x.floor() as usize).min(s.len() - 2);
    let f = x - i as f64;
    let (a, b) = (s[i], s[i + 1]);
    let mix = |p: u8, q: u8| (f64::from(p) + (f64::from(q) - f64::from(p)) * f).round().clamp(0.0, 255.0) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), 0xFF]
}

/// `n` evenly spaced colours for classes, low to high (the class midpoints).
pub fn classes(name: &str, n: usize) -> Vec<Rgba> {
    (0..n).map(|k| sample(name, if n <= 1 { 0.5 } else { k as f64 / (n - 1) as f64 })).collect()
}

/// Whether a unit is a temperature (offer Heat first).
pub fn is_temperature(unit: Option<&str>) -> bool {
    unit.is_some_and(|u| {
        let u = u.trim().to_lowercase();
        u.contains('°') || u == "c" || u == "k" || u == "f" || u.contains("celsius") || u.contains("kelvin") || u.contains("fahrenheit")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_are_the_first_and_last_stops() {
        assert_eq!(sample("viridis", 0.0), VIRIDIS[0]);
        assert_eq!(sample("viridis", 1.0), VIRIDIS[8]);
        assert_eq!(sample("heat", 2.0), HEAT[8]);
        assert_eq!(sample("blues", f64::NAN), BLUES[0]);
        assert_eq!(classes("cividis", 5).len(), 5);
    }

    #[test]
    fn temperatures_are_recognised() {
        assert!(is_temperature(Some("°C")));
        assert!(is_temperature(Some("K")));
        assert!(!is_temperature(Some("m")));
        assert!(!is_temperature(None));
    }
}
