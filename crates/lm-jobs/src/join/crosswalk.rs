//! Bundled crosswalks: standard names and codes for countries (Natural Earth, public domain),
//! US states (Census FIPS and USPS abbreviations) and Chinese provinces (GB/T 2260 codes,
//! Chinese and English names). Each family maps every name or code it knows to one canonical key.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::{code, stripped};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Family {
    Countries,
    UsStates,
    CnProvinces,
}

const COUNTRIES: &str = include_str!("countries.csv");
const US_STATES: &str = include_str!("us_states.csv");
const CN_PROVINCES: &str = include_str!("cn_provinces.csv");

/// name or code (stripped, or as a code) → canonical key, per family. A name two places share
/// in one family is dropped (it would be a guess).
fn index(family: Family) -> &'static HashMap<String, String> {
    static IX: OnceLock<[HashMap<String, String>; 3]> = OnceLock::new();
    let all = IX.get_or_init(|| [build(COUNTRIES), build(US_STATES), build(CN_PROVINCES)]);
    match family {
        Family::Countries => &all[0],
        Family::UsStates => &all[1],
        Family::CnProvinces => &all[2],
    }
}

fn build(csv: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut clash: Vec<String> = Vec::new();
    for line in csv.lines().skip(1) {
        let Some((key, names)) = line.split_once(',') else { continue };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        for name in std::iter::once(key).chain(names.split('|')) {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            for k in [Some(stripped(name)), code(name)].into_iter().flatten() {
                if k.is_empty() {
                    continue;
                }
                match map.get(&k) {
                    Some(existing) if existing != key => clash.push(k),
                    Some(_) => {}
                    None => {
                        map.insert(k, key.to_string());
                    }
                }
            }
        }
    }
    for k in clash {
        map.remove(&k);
    }
    map
}

/// The canonical key `s` stands for in `family`, if it is a name or code the family knows.
pub fn canonical(family: Family, s: &str) -> Option<String> {
    let ix = index(family);
    ix.get(&stripped(s)).or_else(|| code(s).and_then(|c| ix.get(&c))).cloned()
}
