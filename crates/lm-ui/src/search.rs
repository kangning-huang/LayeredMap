//! Toolbar search (Interaction specification › Toolbar; WP1.5): places from the bundled
//! GeoNames index, searched locally as you type. "Your data" (Phase 3) and "Find an action…"
//! (Phase 4) join the list in their phases.

use std::sync::mpsc::{Receiver, channel};

use lm_core::places::{PlaceIndex, zoom_for_population};

/// The bundled city index (GeoNames cities15000, CC BY 4.0).
const CITIES: &str = include_str!("../../../assets/geonames/cities.tsv");
/// Results shown at once.
pub const LIMIT: usize = 8;

/// A place to fly to.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub lon: f64,
    pub lat: f64,
    pub zoom: f64,
    pub label: String,
}

/// One result row as shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The place's name, with its Chinese name when it has one that differs.
    pub title: String,
    /// "Region, Country".
    pub detail: String,
    pub choice: Choice,
}

pub struct Search {
    index: Option<PlaceIndex>,
    loading: Option<Receiver<Result<PlaceIndex, String>>>,
    pub query: String,
    pub rows: Vec<Row>,
    /// The highlighted row (arrow keys).
    pub selected: usize,
    pub open: bool,
    searched: String,
}

impl Search {
    /// Starts parsing the city index on a background thread (about 34,000 rows).
    pub fn new() -> Search {
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new().name("lm-places".into()).spawn(move || {
            let _ = tx.send(PlaceIndex::parse(CITIES));
        });
        if let Err(e) = spawned {
            log::error!("could not start the place index: {e}");
        }
        Search { index: None, loading: Some(rx), query: String::new(), rows: Vec::new(), selected: 0, open: false, searched: String::new() }
    }

    /// Takes the index once it has been parsed.
    fn poll(&mut self) {
        if let Some(rx) = &self.loading
            && let Ok(r) = rx.try_recv()
        {
            match r {
                Ok(ix) => self.index = Some(ix),
                Err(e) => log::error!("place index: {e}"),
            }
            self.loading = None;
            self.searched.clear();
        }
    }

    pub fn ready(&mut self) -> bool {
        self.poll();
        self.index.is_some()
    }

    /// Recomputes the rows when the query changed.
    pub fn update(&mut self) {
        self.poll();
        if self.query == self.searched {
            return;
        }
        self.searched = self.query.clone();
        self.selected = 0;
        self.rows = match &self.index {
            Some(ix) => ix.search(&self.query, LIMIT).into_iter().map(|h| row(h.place)).collect(),
            None => Vec::new(),
        };
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.rows.clear();
        self.searched.clear();
        self.open = false;
    }
}

impl Default for Search {
    fn default() -> Self {
        Search::new()
    }
}

fn row(p: &lm_core::places::Place) -> Row {
    let title = if !p.zh.is_empty() && p.zh != p.name { format!("{} {}", p.name, p.zh) } else { p.name.clone() };
    let detail = match (p.region.is_empty(), p.country.is_empty()) {
        (false, false) if p.region != p.name => format!("{}, {}", p.region, p.country),
        (_, false) => p.country.clone(),
        _ => p.region.clone(),
    };
    Row { title, detail, choice: Choice { lon: p.lon, lat: p.lat, zoom: zoom_for_population(p.population), label: p.name.clone() } }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Search {
        let mut s = Search::new();
        for _ in 0..500 {
            if s.ready() {
                return s;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the place index did not load");
    }

    #[test]
    fn gate_1_searches_find_their_cities() {
        let mut s = ready();
        for (q, want) in [("Shanghai", "Shanghai"), ("成都", "Chengdu"), ("Lisbon", "Lisbon")] {
            s.query = q.into();
            s.update();
            let first = s.rows.first().unwrap();
            assert_eq!(first.choice.label, want, "{q}");
            assert!(!first.detail.is_empty());
        }
        s.query = "Shanghai".into();
        s.update();
        assert!(s.rows[0].title.contains("上海"), "the Chinese name shows: {}", s.rows[0].title);
        assert!(s.rows.len() <= LIMIT);
    }

    #[test]
    fn an_empty_query_shows_nothing() {
        let mut s = ready();
        s.query = "   ".into();
        s.update();
        assert!(s.rows.is_empty());
    }
}
