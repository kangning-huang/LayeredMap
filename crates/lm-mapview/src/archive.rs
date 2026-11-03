//! The online basemap archive. Protomaps publishes a planet build each day at
//! `build.protomaps.com/<YYYYMMDD>.pmtiles` and deletes each one after about a week, so a pinned
//! date stops answering (and the map falls back to the offline outline). At start-up the newest
//! build that answers is found off the frame; until then the pinned one is read.

use std::sync::Mutex;

/// Where the daily builds live.
pub const BUILDS: &str = "https://build.protomaps.com/";
/// The list of builds (it also names deleted ones, so each candidate is checked).
pub const METADATA: &str = "https://build-metadata.protomaps.dev/builds.json";
/// The basemap schema the built-in style draws: builds of another major version are skipped.
const SCHEMA_MAJOR: &str = "4.";
/// How many days back to try when the list can't be read.
const DAYS_BACK: i64 = 7;

#[derive(Clone, Debug, PartialEq)]
enum State {
    Idle,
    Looking,
    Found(String),
    Failed,
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

fn state() -> State {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn set(s: State) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

/// The archive to read: the newest build found, else `pinned`.
pub fn url(pinned: &str) -> String {
    match state() {
        State::Found(u) => u,
        _ => pinned.to_string(),
    }
}

/// The newest build, once found.
pub fn found() -> Option<String> {
    match state() {
        State::Found(u) => Some(u),
        _ => None,
    }
}

/// Whether the newest build is still being looked for (a failing pinned build is expected then).
pub fn looking() -> bool {
    state() == State::Looking
}

/// The app asked for the lookup (it may have been put off while offline).
static WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the app asked for the lookup: going back online then starts it, if it hasn't run.
pub fn wanted() -> bool {
    WANTED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Starts looking for the newest build, once per run; `wake` is called when the answer is in.
/// In offline mode it waits: calling it again once online starts it.
#[cfg(not(target_arch = "wasm32"))]
pub fn start(wake: impl Fn() + Send + 'static) {
    WANTED.store(true, std::sync::atomic::Ordering::SeqCst);
    if lm_net::offline() {
        return;
    }
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if *s != State::Idle {
            return;
        }
        *s = State::Looking;
    }
    std::thread::Builder::new()
        .name("lm-basemap-archive".into())
        .spawn(move || {
            let found = native::newest();
            match &found {
                Some(u) => log::info!("basemap archive: {u}"),
                None => log::warn!("basemap archive: no daily build answered"),
            }
            set(found.map_or(State::Failed, State::Found));
            wake();
        })
        .map_err(|e| log::warn!("basemap archive: {e}"))
        .ok();
}

/// The web build: the page looks for the newest build itself (it has no threads and needs a
/// same-origin proxy, apps/nimblemap-web) and reports it with [`set_found`].
#[cfg(target_arch = "wasm32")]
pub fn start(_wake: impl Fn() + Send + 'static) {
    WANTED.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if *s == State::Idle {
        *s = State::Looking;
    }
}

/// The web build: the newest build that answered (`None`: none did, the pinned one is read).
#[cfg(target_arch = "wasm32")]
pub fn set_found(url: Option<String>) {
    set(url.map_or(State::Failed, State::Found));
}

/// The builds to try, newest first: the list's newest builds of the schema the style draws, then
/// the last week's dates (UTC), in case the list can't be read.
pub fn candidates(metadata: Option<&str>, today: i64) -> Vec<String> {
    let mut keys: Vec<String> = metadata
        .and_then(|m| serde_json::from_str::<Vec<serde_json::Value>>(m).ok())
        .unwrap_or_default()
        .iter()
        .filter(|b| b.get("version").and_then(|v| v.as_str()).is_none_or(|v| v.starts_with(SCHEMA_MAJOR)))
        .filter_map(|b| b.get("key")?.as_str().map(str::to_string))
        .filter(|k| k.len() == 16 && k.ends_with(".pmtiles") && k[..8].bytes().all(|c| c.is_ascii_digit()))
        .collect();
    keys.sort_unstable_by(|a, b| b.cmp(a));
    keys.truncate(3);
    for d in 0..=DAYS_BACK {
        let k = format!("{}.pmtiles", ymd(today - d));
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys.into_iter().map(|k| format!("{BUILDS}{k}")).collect()
}

/// A day number (days since 1970-01-01) as `YYYYMMDD` (proleptic Gregorian).
pub fn ymd(days: i64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}{m:02}{d:02}")
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::time::Duration;

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(8))).build().into()
    }

    /// The newest build that answers, if any. Each request asks the network gate first.
    pub(super) fn newest() -> Option<String> {
        let allowed = || lm_net::allowed(lm_net::Purpose::Basemap);
        let agent = agent();
        if !allowed() {
            return None;
        }
        let metadata = agent.get(super::METADATA).call().ok().and_then(|mut r| r.body_mut().read_to_string().ok());
        let today = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).ok()?.as_secs() as i64 / 86_400;
        super::candidates(metadata.as_deref(), today).into_iter().take_while(|_| allowed()).find(|u| agent.head(u).call().is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_yyyymmdd() {
        assert_eq!(ymd(0), "19700101");
        assert_eq!(ymd(20_743), "20261017");
        assert_eq!(ymd(19_782), "20240229");
    }

    #[test]
    fn newest_builds_of_the_drawn_schema_come_first() {
        let m = r#"[{"key":"20261014.pmtiles","version":"4.15.2"},{"key":"20261016.pmtiles","version":"4.15.2"},
            {"key":"20261017.pmtiles","version":"5.0.0"},{"key":"notes.txt"},{"key":"20261015.pmtiles","version":"4.15.2"}]"#;
        let c = candidates(Some(m), 20_743);
        assert_eq!(c[0], format!("{BUILDS}20261016.pmtiles"));
        assert_eq!(c[1], format!("{BUILDS}20261015.pmtiles"));
        assert_eq!(c[2], format!("{BUILDS}20261014.pmtiles"));
        // Then the last week's dates, without repeats; never the other schema's build first.
        assert_eq!(c[3], format!("{BUILDS}20261017.pmtiles"));
        assert_eq!(c.iter().filter(|u| u.ends_with("20261016.pmtiles")).count(), 1);
        assert_eq!(c.len(), 3 + 8 - 3);
    }

    /// Network: the newest build answers (`cargo test -p lm-mapview archive -- --ignored`).
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn the_newest_build_answers() {
        let u = native::newest().expect("a daily build");
        eprintln!("{u}");
        assert!(u.starts_with(BUILDS) && u.ends_with(".pmtiles"));
    }

    #[test]
    fn without_the_list_the_last_week_is_tried() {
        let c = candidates(None, 20_743);
        assert_eq!(c.len(), 8);
        assert!(c[0].ends_with("20261017.pmtiles") && c[7].ends_with("20261010.pmtiles"));
        assert_eq!(candidates(Some("not json"), 20_743), c);
    }
}
