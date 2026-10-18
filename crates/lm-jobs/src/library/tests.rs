use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use super::*;

const WAIT: Duration = Duration::from_secs(20);

/// A fresh folder under the system temporary directory, removed when dropped.
struct Temp(PathBuf);

impl Temp {
    fn new(tag: &str) -> Temp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("lm-library-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn file_url(p: &Path) -> String {
    format!("file://{}", p.display())
}

fn wake() -> Wake {
    Arc::new(|| {})
}

/// A dataset of one file served from `urls`.
fn dataset(id: &str, name: &str, bytes: &[u8], urls: Vec<String>) -> Dataset {
    Dataset {
        id: id.into(),
        title: Text { en: "Test".into(), zh: "测试".into() },
        description: Text { en: "A test file".into(), zh: "测试文件".into() },
        kind: Kind::Points,
        extent: [-180.0, -90.0, 180.0, 90.0],
        license: License { id: "CC0-1.0".into(), name: "CC0".into(), url: "https://creativecommons.org/publicdomain/zero/1.0/".into() },
        source: "https://example.org".into(),
        citation: "Test".into(),
        date: "2026".into(),
        attribution: None,
        thumbnail: None,
        files: vec![DataFile { name: name.into(), urls, size: bytes.len() as u64, sha256: sha(bytes) }],
    }
}

/// Some bytes larger than a few chunks.
fn payload(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

/// Files left in a folder, recursively, as names relative to it.
fn files_in(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push(e.path().strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

/// A minimal HTTP/1.1 server on localhost: `GET /<name>` answers the named body, anything else
/// 404. Serves until the test ends.
fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);
            loop {
                let mut h = String::new();
                if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
            let mut s = stream;
            match files.iter().find(|(n, _)| *n == path) {
                Some((_, body)) => {
                    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = s.write_all(body);
                }
                None => {
                    let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                }
            }
        }
    });
    format!("http://{addr}")
}

const CATALOG: &str = r#"{
  "format": "layeredmap-library",
  "version": 1,
  "updated": "2026-10-18",
  "datasets": [
    {
      "id": "world-cities",
      "title": { "en": "World cities", "zh": "世界城市" },
      "description": { "en": "Cities and towns.", "zh": "城市和城镇。" },
      "kind": "points",
      "extent": [-180, -90, 180, 90],
      "licence": { "id": "public-domain", "name": "Public domain", "url": "https://www.naturalearthdata.com/about/terms-of-use/" },
      "source": "https://www.naturalearthdata.com/",
      "citation": "Made with Natural Earth.",
      "date": "v5.1.2",
      "thumbnail": "thumbs/cities.png",
      "files": [
        { "name": "cities.geojson", "urls": ["https://mirror.example.cn/cities.geojson", "../examples/cities.geojson"], "size": 100,
          "sha256": "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789" }
      ]
    },
    {
      "id": "districts",
      "title": { "en": "Districts", "zh": "区" },
      "description": { "en": "Areas.", "zh": "区域。" },
      "kind": "areas",
      "extent": [120.8, 30.6, 122.2, 31.9],
      "license": { "id": "CC-BY-4.0", "name": "CC BY 4.0", "url": "https://creativecommons.org/licenses/by/4.0/" },
      "source": "https://example.org/d",
      "citation": "Example (2026). Districts.",
      "date": "2026",
      "attribution": "© Example",
      "files": [
        { "name": "d.shp", "urls": ["/data/d.shp"], "size": 10, "sha256": "0000000000000000000000000000000000000000000000000000000000000000" },
        { "name": "d.dbf", "urls": ["d.dbf"], "size": 5, "sha256": "1111111111111111111111111111111111111111111111111111111111111111" }
      ]
    }
  ]
}"#;

#[test]
fn a_catalog_is_read_and_its_urls_resolved() {
    let c = parse(CATALOG, "https://data.example.cn/library/catalog.json").unwrap();
    assert!(c.skipped.is_empty(), "{:?}", c.skipped);
    assert_eq!(c.updated.as_deref(), Some("2026-10-18"));
    assert_eq!(c.datasets.len(), 2);
    let cities = c.get("world-cities").unwrap();
    assert_eq!(cities.kind, Kind::Points);
    assert_eq!(cities.title.get("zh-Hans"), "世界城市");
    assert_eq!(cities.title.get("en"), "World cities");
    // "licence" is read as "license".
    assert_eq!(cities.license.name, "Public domain");
    assert_eq!(cities.thumbnail.as_deref(), Some("https://data.example.cn/library/thumbs/cities.png"));
    assert_eq!(cities.files[0].urls, ["https://mirror.example.cn/cities.geojson", "https://data.example.cn/examples/cities.geojson"]);
    // Checksums are kept in lowercase.
    assert!(cities.files[0].sha256.starts_with("abcdef"));
    let d = c.get("districts").unwrap();
    assert_eq!(d.size(), 15);
    assert_eq!(d.attribution.as_deref(), Some("© Example"));
    assert_eq!(d.files[0].urls, ["https://data.example.cn/data/d.shp"]);
    assert_eq!(d.files[1].urls, ["https://data.example.cn/library/d.dbf"]);
    assert!(d.matches("区"));
    assert!(d.matches("cc by areas"));
    assert!(!d.matches("cities"));
}

#[test]
fn relative_urls_resolve_against_files_too() {
    assert_eq!(resolve("file:///repo/fixtures/library/catalog.json", "../../assets/x.tif"), "file:///repo/assets/x.tif");
    assert_eq!(resolve("/repo/fixtures/library/catalog.json", "thumbs/a.png"), "/repo/fixtures/library/thumbs/a.png");
    assert_eq!(resolve("https://h.cn/a/b.json", "https://other.org/c"), "https://other.org/c");
    assert_eq!(resolve("https://h.cn", "c.json"), "https://h.cn/c.json");
    assert_eq!(local_path("file:///Users/me/My%20Data/a.json"), Some(PathBuf::from("/Users/me/My Data/a.json")));
    assert_eq!(local_path("file://localhost/tmp/a.json"), Some(PathBuf::from("/tmp/a.json")));
    assert_eq!(local_path("https://h.cn/a.json"), None);
}

#[test]
fn bad_entries_are_skipped_and_bad_catalogs_refused() {
    let mut v: serde_json::Value = serde_json::from_str(CATALOG).unwrap();
    let good = v["datasets"][0].clone();
    let mut bad = Vec::new();
    let edits: [fn(&mut serde_json::Value); 8] = [
        |d: &mut serde_json::Value| d["title"]["zh"] = "".into(),
        |d: &mut serde_json::Value| d["files"][0]["name"] = "../../etc/passwd".into(),
        |d: &mut serde_json::Value| d["files"][0]["sha256"] = "abc".into(),
        |d: &mut serde_json::Value| d["id"] = "Bad Id".into(),
        |d: &mut serde_json::Value| d["files"] = serde_json::json!([]),
        |d: &mut serde_json::Value| d["extent"] = serde_json::json!([10, 0, 5, 1]),
        |d: &mut serde_json::Value| d["kind"] = "volcano".into(),
        |d: &mut serde_json::Value| {
            d.as_object_mut().unwrap().remove("citation");
        },
    ];
    for (i, edit) in edits.iter().enumerate() {
        let mut d = good.clone();
        d["id"] = format!("bad-{i}").into();
        edit(&mut d);
        bad.push(d);
    }
    // A repeated id is dropped too.
    bad.push(good.clone());
    v["datasets"].as_array_mut().unwrap().extend(bad);
    let c = parse(&v.to_string(), "https://h.cn/catalog.json").unwrap();
    assert_eq!(c.datasets.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(), ["world-cities", "districts"]);
    assert_eq!(c.skipped.len(), 9, "{:#?}", c.skipped);

    let other = CATALOG.replace("layeredmap-library", "something-else");
    assert!(matches!(parse(&other, "x"), Err(LibraryError::BadCatalog(_))));
    let newer = CATALOG.replace("\"version\": 1", "\"version\": 2");
    assert!(matches!(parse(&newer, "x"), Err(LibraryError::BadCatalog(_))));
    assert!(matches!(parse("not json", "x"), Err(LibraryError::BadCatalog(_))));
}

/// The local test catalog in the repository parses, and each entry's checksum matches its file.
#[test]
fn the_local_test_catalog_matches_its_files() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/catalog.json");
    let url = file_url(&path);
    let c = parse(&std::fs::read_to_string(&path).unwrap(), &url).unwrap();
    assert!(c.skipped.is_empty(), "{:?}", c.skipped);
    assert!((2..=3).contains(&c.datasets.len()));
    for d in &c.datasets {
        for f in &d.files {
            let p = local_path(&f.urls[0]).unwrap();
            let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert_eq!(bytes.len() as u64, f.size, "{}", f.name);
            assert_eq!(sha(&bytes), f.sha256, "{}", f.name);
        }
        let t = local_path(d.thumbnail.as_deref().unwrap()).unwrap();
        assert!(t.is_file(), "{}", t.display());
    }
}

#[test]
fn a_file_url_downloads_into_the_cache() {
    let src = Temp::new("src");
    let cache = Temp::new("cache");
    let bytes = payload(300_000);
    std::fs::write(src.0.join("pop.tif"), &bytes).unwrap();
    let d = dataset("pop", "pop.tif", &bytes, vec![file_url(&src.0.join("pop.tif"))]);
    assert!(!is_cached(&cache.0, &d));
    let mut job = download(&d, cache.0.clone(), wake());
    let path = job.wait(WAIT).expect("finished").unwrap();
    assert_eq!(path, dataset_dir(&cache.0, "pop").join("pop.tif"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(job.done(), job.total);
    assert!(is_cached(&cache.0, &d));
    assert_eq!(files_in(&cache.0), ["datasets/pop/pop.tif", "datasets/pop/pop.tif.sha256"]);
}

#[test]
fn http_downloads_with_a_mirror_and_plain_errors() {
    let bytes = payload(200_000);
    let base = serve(vec![("a.geojson", bytes.clone())]);
    let cache = Temp::new("http");
    // The first URL is gone; the mirror answers.
    let d = dataset("a", "a.geojson", &bytes, vec![format!("{base}/gone.geojson"), format!("{base}/a.geojson")]);
    let p = fetch_dataset(&d, &cache.0, &Progress::default(), &|| {}).unwrap();
    assert_eq!(std::fs::read(p).unwrap(), bytes);

    let missing = dataset("b", "b.geojson", &bytes, vec![format!("{base}/b.geojson")]);
    let e = fetch_dataset(&missing, &cache.0, &Progress::default(), &|| {}).unwrap_err();
    assert_eq!(e, LibraryError::NotFound("b.geojson".into()));
    assert_eq!(e.to_string(), "The data library no longer has b.geojson. Try again later.");

    // Nothing listens on port 9 (discard) on a test machine: a plain "couldn't reach" message.
    let unreachable = dataset("c", "c.geojson", &bytes, vec!["http://127.0.0.1:9/c.geojson".into()]);
    let e = fetch_dataset(&unreachable, &cache.0, &Progress::default(), &|| {}).unwrap_err();
    assert!(matches!(e, LibraryError::Network(_)), "{e:?}");
    assert!(e.to_string().starts_with("Couldn't reach the data library."));
}

#[test]
fn a_checksum_mismatch_keeps_nothing() {
    let src = Temp::new("src");
    let cache = Temp::new("sum");
    let bytes = payload(150_000);
    std::fs::write(src.0.join("x.csv"), &bytes).unwrap();
    let mut d = dataset("x", "x.csv", &bytes, vec![file_url(&src.0.join("x.csv"))]);
    d.files[0].sha256 = sha(b"something else");
    let e = fetch_dataset(&d, &cache.0, &Progress::default(), &|| {}).unwrap_err();
    assert_eq!(e, LibraryError::Checksum("x.csv".into()));
    assert!(e.to_string().contains("didn't match the library's checksum, so it wasn't opened"));
    assert!(files_in(&cache.0).is_empty(), "{:?}", files_in(&cache.0));
    // A file larger than the catalog says is stopped, and nothing is kept either.
    let mut big = dataset("y", "y.csv", &bytes, vec![file_url(&src.0.join("x.csv"))]);
    big.files[0].size = 1000;
    assert_eq!(fetch_dataset(&big, &cache.0, &Progress::default(), &|| {}).unwrap_err(), LibraryError::TooBig("y.csv".into()));
    assert!(files_in(&cache.0).is_empty(), "{:?}", files_in(&cache.0));
}

#[test]
fn a_cached_dataset_opens_without_the_network() {
    let src = Temp::new("src");
    let cache = Temp::new("offline");
    let bytes = payload(100_000);
    std::fs::write(src.0.join("e.tif"), &bytes).unwrap();
    let d = dataset("e", "e.tif", &bytes, vec![file_url(&src.0.join("e.tif"))]);
    let first = fetch_dataset(&d, &cache.0, &Progress::default(), &|| {}).unwrap();
    // The source is gone and the host can't be reached: the cache still serves it.
    drop(src);
    let mut offline = d.clone();
    offline.files[0].urls = vec!["http://127.0.0.1:9/e.tif".into(), "file:///nonexistent/e.tif".into()];
    let mut job = download(&offline, cache.0.clone(), wake());
    assert_eq!(job.wait(WAIT).expect("finished").unwrap(), first);
    assert_eq!(job.done(), job.total);
    // A changed file in the catalog (new checksum) is not served from the old copy.
    let mut changed = offline.clone();
    changed.files[0].sha256 = sha(b"version 2");
    assert!(!is_cached(&cache.0, &changed));
    assert!(fetch_dataset(&changed, &cache.0, &Progress::default(), &|| {}).is_err());
}

#[test]
fn cancel_stops_and_leaves_no_partial_file() {
    let src = Temp::new("src");
    let cache = Temp::new("cancel");
    let bytes = payload(1_000_000);
    std::fs::write(src.0.join("big.tif"), &bytes).unwrap();
    let d = dataset("big", "big.tif", &bytes, vec![file_url(&src.0.join("big.tif"))]);
    // Cancelled after the first chunk.
    let progress = Progress::default();
    let tick = || progress.cancel.store(true, Ordering::Relaxed);
    assert_eq!(fetch_dataset(&d, &cache.0, &progress, &tick).unwrap_err(), LibraryError::Cancelled);
    assert!(progress.done.load(Ordering::Relaxed) < bytes.len() as u64);
    assert!(files_in(&cache.0).is_empty(), "{:?}", files_in(&cache.0));
    assert!(!is_cached(&cache.0, &d));

    // A background download reports Cancelled as soon as Cancel is pressed, once.
    let mut job = download(&d, cache.0.clone(), wake());
    job.cancel();
    assert_eq!(job.poll(), Some(Err(LibraryError::Cancelled)));
    assert_eq!(job.poll(), None);
    // Its thread notices and cleans up; give it a moment.
    for _ in 0..200 {
        if files_in(&cache.0).iter().all(|f| !f.ends_with(".part")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(files_in(&cache.0).iter().all(|f| !f.ends_with(".part")), "{:?}", files_in(&cache.0));
}

#[test]
fn the_catalog_and_thumbnails_load_and_a_saved_copy_is_used_offline() {
    let src = Temp::new("src");
    let cache = Temp::new("catalog");
    let bytes = payload(1000);
    std::fs::write(src.0.join("a.csv"), &bytes).unwrap();
    std::fs::write(src.0.join("a.png"), b"png bytes").unwrap();
    let mut d = dataset("a", "a.csv", &bytes, vec!["a.csv".into()]);
    d.thumbnail = Some("a.png".into());
    let json = serde_json::json!({ "format": FORMAT, "version": 1, "datasets": [d] }).to_string();
    std::fs::write(src.0.join("catalog.json"), json).unwrap();
    let url = file_url(&src.0.join("catalog.json"));

    let job = load_catalog(url.clone(), cache.0.clone(), wake());
    match job.wait(WAIT) {
        Some(CatalogEvent::Loaded { catalog, saved_copy, cached }) => {
            assert!(!saved_copy);
            assert!(cached.is_empty());
            assert_eq!(catalog.datasets[0].files[0].urls[0], file_url(&src.0.join("a.csv")));
        }
        other => panic!("{other:?}"),
    }
    match job.wait(WAIT) {
        Some(CatalogEvent::Thumbnail { id, png }) => assert_eq!((id.as_str(), png.as_slice()), ("a", b"png bytes".as_slice())),
        other => panic!("{other:?}"),
    }
    let ds = parse(&std::fs::read_to_string(src.0.join("catalog.json")).unwrap(), &url).unwrap().datasets.remove(0);
    fetch_dataset(&ds, &cache.0, &Progress::default(), &|| {}).unwrap();

    // The host is gone: the saved copy, the cached thumbnail and the downloaded dataset remain.
    drop(src);
    let job = load_catalog(url, cache.0.clone(), wake());
    match job.wait(WAIT) {
        Some(CatalogEvent::Loaded { saved_copy, cached, .. }) => {
            assert!(saved_copy);
            assert!(cached.contains("a"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(job.wait(WAIT), Some(CatalogEvent::Thumbnail { .. })));

    // Never reached and never saved: a plain failure.
    let job = load_catalog("file:///nonexistent/catalog.json".into(), cache.0.clone(), wake());
    match job.wait(WAIT) {
        Some(CatalogEvent::Failed(e)) => assert!(matches!(e, LibraryError::NotFound(_)), "{e:?}"),
        other => panic!("{other:?}"),
    }
}
