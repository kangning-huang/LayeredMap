//! The browser shell: the eframe web runner, files in and out, prefs in `localStorage`, the
//! map's downloads through `fetch` with a Cache API copy, and the page's own controls.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use lm_core::phase::Flags;
use lm_ui::{Command, FileRequest, Prefs, Shell, System, ThemePref, Units};
use wasm_bindgen::JsCast as _;

use crate::files;

const CANVAS_ID: &str = "nimblemap_canvas";
const LOADING_ID: &str = "nimblemap_loading";
/// The Cache API store for basemap tiles and imagery (survives reloads; the browser may evict it).
const TILE_CACHE: &str = "nimblemap-tiles-v1";
/// Larger responses aren't kept in the tile cache.
const TILE_CACHE_MAX: usize = 4 << 20;

/// What finished reading or fetching in the background, for the next frame.
enum Incoming {
    Open(Vec<PathBuf>),
    Notice(String),
}

type Inbox = Rc<RefCell<Vec<Incoming>>>;

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    // Map tiles and imagery come through the browser's fetch (the map repaints when they land).
    rustgis_tiles::cache::set_fetcher(|url, done| {
        wasm_bindgen_futures::spawn_local(async move { done(fetch_tile(&url).await) });
    });
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let mut options = eframe::WebOptions::default();
        if query().contains("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let result = eframe::WebRunner::new().start(canvas, options, Box::new(|cc| Ok(Box::new(WebApp::new(cc))))).await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!(
                    "<p>NimbleMap couldn't start: {e:?}</p><p>It needs a browser with WebGPU or WebGL2 (a recent Chrome, Edge, Firefox or Safari).</p>"
                )),
            }
        }
    });
}

struct WebApp {
    shell: Shell,
    system: System,
    inbox: Inbox,
    next_batch: u64,
    /// The prefs as last stored, so they are written only when they change.
    saved: String,
    next_save: f64,
    first_map: bool,
    chinese_requested: bool,
    korean_requested: bool,
    fonts_changed: Rc<std::cell::Cell<bool>>,
}

impl WebApp {
    fn new(cc: &eframe::CreationContext<'_>) -> WebApp {
        let ctx = &cc.egui_ctx;
        let system =
            System { dark_mode: ctx.system_theme() == Some(egui::Theme::Dark), reduce_motion: media("(prefers-reduced-motion: reduce)") };
        let mut prefs = load_prefs();
        let q = query();
        // `?lang=de` picks the interface language for this visit (the browser's otherwise).
        if let Some(code) = param(&q, "lang").filter(|c| lm_core::i18n::Lang::from_code(c).is_some()) {
            prefs.language = code;
        }
        // `?view=lon,lat,zoom` starts there (links and screenshots).
        if let Some(v) = param(&q, "view").and_then(|v| parse_view(&v)) {
            prefs.view = Some(v);
        }
        let mut shell = Shell::new(ctx, Flags::from_env(None), prefs, system);
        shell.set_system_languages(browser_languages());
        shell.developer = false;
        lm_ui::basemap::start_archive_lookup(ctx);
        find_basemap_build(ctx.clone());
        let mut app = WebApp {
            shell,
            system,
            inbox: Inbox::default(),
            next_batch: 0,
            saved: String::new(),
            next_save: 0.0,
            first_map: false,
            chinese_requested: false,
            korean_requested: false,
            fonts_changed: Rc::default(),
        };
        app.saved = prefs_text(&app.shell.prefs_to_save());
        // `?example=storms` opens an example at once (links and screenshots).
        if let Some(id) = param(&q, "example").filter(|id| lm_ui::examples::EXAMPLES.iter().any(|e| e.id == id)) {
            app.shell.command(ctx, Command::OpenExample(id));
        }
        app
    }

    /// Holds the files of one drop or pick in memory and opens them as one batch.
    fn take_batch(inbox: &Inbox, ctx: &egui::Context, batch: u64, read: Vec<(String, Vec<u8>)>) {
        let dir = files::batch_dir(batch);
        let names: Vec<String> = read.iter().map(|(n, _)| files::clean_name(n)).collect();
        for ((_, bytes), name) in read.into_iter().zip(&names) {
            lm_jobs::vfs::put(dir.join(name), bytes);
        }
        let open: Vec<PathBuf> = files::to_open(&names).into_iter().map(|n| dir.join(n)).collect();
        if !open.is_empty() {
            inbox.borrow_mut().push(Incoming::Open(open));
            ctx.request_repaint();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if dropped.is_empty() {
            return;
        }
        self.next_batch += 1;
        let (batch, inbox, ctx) = (self.next_batch, self.inbox.clone(), ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let mut read = Vec::new();
            for f in dropped {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                match f.bytes_async().await {
                    Ok(bytes) => read.push((name, bytes)),
                    Err(e) => inbox.borrow_mut().push(Incoming::Notice(format!("{name}: {e}"))),
                }
            }
            WebApp::take_batch(&inbox, &ctx, batch, read);
        });
    }

    fn handle_requests(&mut self, ctx: &egui::Context) {
        for req in self.shell.take_file_requests() {
            match req {
                FileRequest::Open => {
                    self.next_batch += 1;
                    let (batch, inbox, ctx) = (self.next_batch, self.inbox.clone(), ctx.clone());
                    let dialog = rfd::AsyncFileDialog::new().add_filter("Map data", files::PICK_EXTS);
                    wasm_bindgen_futures::spawn_local(async move {
                        let picked = dialog.pick_files().await.unwrap_or_default();
                        let mut read = Vec::new();
                        for f in picked {
                            read.push((f.file_name(), f.read().await));
                        }
                        WebApp::take_batch(&inbox, &ctx, batch, read);
                    });
                }
                FileRequest::SaveText { suggested_name, contents, .. } => save(&suggested_name, contents.as_bytes()),
                FileRequest::SaveBytes { suggested_name, bytes, .. } => save(&suggested_name, &bytes),
                FileRequest::Fetch(paths) => {
                    let (inbox, ctx2) = (self.inbox.clone(), ctx.clone());
                    self.shell.notify(ctx, lm_core::tl!("Loading…"));
                    wasm_bindgen_futures::spawn_local(async move {
                        for p in &paths {
                            if lm_jobs::vfs::is_held(p) {
                                continue;
                            }
                            let Some(url) = files::example_url(p) else { continue };
                            match fetch(&url, None).await {
                                Ok(bytes) => lm_jobs::vfs::put(p, bytes),
                                Err(e) => {
                                    inbox.borrow_mut().push(Incoming::Notice(format!("{url}: {e}")));
                                    ctx2.request_repaint();
                                    return;
                                }
                            }
                        }
                        inbox.borrow_mut().push(Incoming::Open(paths));
                        ctx2.request_repaint();
                    });
                }
                FileRequest::SaveAs { .. } | FileRequest::SavePackage { .. } | FileRequest::SaveDocument | FileRequest::OpenDocument => {
                    self.shell.notify(ctx, "Saving a map file comes to the web version soon. Share › Map image works now.");
                }
                FileRequest::Locate { file_name, .. } => {
                    self.shell.notify(ctx, format!("{file_name}: drop the file on the map to add it again."));
                }
            }
        }
    }

    fn store_prefs(&mut self, t: f64) {
        if t < self.next_save {
            return;
        }
        self.next_save = t + 2.0;
        let text = prefs_text(&self.shell.prefs_to_save());
        if text != self.saved {
            if let Some(s) = storage() {
                let p = self.shell.prefs_to_save();
                for (k, v) in prefs_pairs(&p) {
                    let _ = s.set_item(k, &v);
                }
            }
            self.saved = text;
        }
    }
}

impl eframe::App for WebApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.system.dark_mode = ctx.system_theme().map_or(self.system.dark_mode, |t| t == egui::Theme::Dark);
        // Background work (imports, analysis previews, the table's sorting) runs here, between
        // frames: the page has one thread.
        let started = web_time::Instant::now();
        loop {
            let left = lm_core::task::pump(1);
            if left == 0 {
                break;
            }
            if started.elapsed().as_millis() > 40 {
                ctx.request_repaint();
                break;
            }
        }
        self.handle_drops(ctx);
        let incoming = std::mem::take(&mut *self.inbox.borrow_mut());
        for i in incoming {
            match i {
                Incoming::Open(paths) => self.shell.command(ctx, Command::Open(paths)),
                Incoming::Notice(n) => self.shell.notify(ctx, n),
            }
        }
        self.handle_requests(ctx);
        let t = ctx.input(|i| i.time);
        self.store_prefs(t);
        self.fonts(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui, self.system);
        self.menu(ui.ctx());
        if !self.first_map && self.shell.map_view().renders > 0 {
            // Time to the first map, for docs/lm/web-app.md (window.__nimblemap_first_map, ms since navigation).
            self.first_map = true;
            if let Some(ms) = web_sys::window().and_then(|w| w.performance()).map(|p| p.now()) {
                log::info!("first map frame {ms:.0} ms after navigation");
                if let Some(w) = web_sys::window() {
                    let _ = js_sys::Reflect::set(&w, &"__nimblemap_first_map".into(), &ms.into());
                }
            }
        }
    }
}

/// Noto Sans KR (SIL OFL), fetched only when the interface is in Korean: the desktop app reads
/// the system's Korean face, which a page can't. Pinned to a google/fonts commit.
const KOREAN_FACE_URL: &str =
    "https://cdn.jsdelivr.net/gh/google/fonts@45e3d5269e4a17d823f6d84470bc084dc5ae46f6/ofl/notosanskr/NotoSansKR%5Bwght%5D.ttf";

impl WebApp {
    /// Downloads a font file and registers it; the fonts are rebuilt on the next frame.
    fn fetch_font(&self, ctx: &egui::Context, url: &'static str, path: &'static str, label_font: bool) {
        let (changed, ctx) = (self.fonts_changed.clone(), ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            match fetch(url, None).await {
                Ok(bytes) => {
                    if label_font && !lm_ui::mapview::add_label_font(bytes.clone()) {
                        log::warn!("{url}: not usable for map labels");
                    }
                    lm_ui::theme::register_font_file(path, bytes);
                    changed.set(true);
                }
                Err(e) => log::warn!("{url} couldn't be downloaded: {e}"),
            }
            ctx.request_repaint();
        });
    }

    /// Chinese (Noto Sans SC, from the site) for every language, after the first frame; Korean
    /// (Noto Sans KR) only when the interface is in Korean. The fonts are rebuilt when one lands.
    fn fonts(&mut self, ctx: &egui::Context) {
        if !self.chinese_requested {
            self.chinese_requested = true;
            self.fetch_font(ctx, "fonts/NotoSansSC-Regular.otf", lm_ui::theme::WEB_NOTO_SC, true);
            wasm_bindgen_futures::spawn_local(async move {
                match fetch("geonames/cities.tsv", None).await.map(String::from_utf8) {
                    Ok(Ok(tsv)) => lm_ui::search::provide_cities(tsv),
                    Ok(Err(e)) => log::warn!("the place index isn't text: {e}"),
                    Err(e) => log::warn!("the place index couldn't be downloaded: {e}"),
                }
            });
        }
        if !self.korean_requested && self.shell.language().cjk() == Some(lm_core::i18n::Cjk::Korean) {
            self.korean_requested = true;
            self.fetch_font(ctx, KOREAN_FACE_URL, lm_ui::theme::WEB_KOREAN_FACE, false);
        }
        if self.fonts_changed.replace(false) {
            ctx.set_fonts(lm_ui::theme::fonts_for(self.shell.language().cjk()));
        }
    }

    /// The desktop menu bar's commands that work here, in one button beside the app's name.
    fn menu(&mut self, ctx: &egui::Context) {
        use lm_core::tl;
        let mut cmd = None;
        let mut help = false;
        egui::Area::new(egui::Id::new("nm-web-menu")).order(egui::Order::Foreground).fixed_pos(egui::pos2(142.0, 12.0)).show(ctx, |ui| {
            let icon = egui::RichText::new(egui_phosphor_list()).size(18.0);
            ui.menu_button(icon, |ui| {
                ui.set_min_width(200.0);
                if ui.button(tl!("Open…")).clicked() {
                    cmd = Some(Command::RequestOpen);
                }
                ui.menu_button(tl!("Open Example"), |ui| {
                    for e in lm_ui::examples::EXAMPLES {
                        if ui.button(lm_core::i18n::t(e.title)).clicked() {
                            cmd = Some(Command::OpenExample(e.id.to_string()));
                        }
                    }
                });
                if ui.button(tl!("Export Map Image…")).clicked() {
                    cmd = Some(Command::ExportMapImage);
                }
                if ui.button(tl!("Export Data…")).clicked() {
                    cmd = Some(Command::ExportData);
                }
                if ui.button(tl!("Remove All Layers")).clicked() {
                    cmd = Some(Command::RemoveAllLayers);
                }
                ui.separator();
                if ui.button(tl!("Zoom to All Data")).clicked() {
                    cmd = Some(Command::ZoomToAll);
                }
                ui.menu_button(tl!("View"), |ui| {
                    for (label, t) in [
                        (tl!("Use System Appearance"), ThemePref::System),
                        (tl!("Light"), ThemePref::Light),
                        (tl!("Dark"), ThemePref::Dark),
                    ] {
                        if ui.radio(self.shell.prefs.theme == t, label).clicked() {
                            cmd = Some(Command::SetTheme(t));
                        }
                    }
                    ui.separator();
                    for (label, u) in [(tl!("Metric Units"), Units::Metric), (tl!("Imperial Units"), Units::Imperial)] {
                        if ui.radio(self.shell.prefs.units == u, label).clicked() {
                            cmd = Some(Command::SetUnits(u));
                        }
                    }
                });
                ui.menu_button(tl!("Language"), |ui| {
                    let current = self.shell.prefs.language.clone();
                    if ui.radio(current == lm_core::i18n::AUTO, tl!("System Language")).clicked() {
                        cmd = Some(Command::SetLanguage(lm_core::i18n::AUTO.into()));
                    }
                    ui.separator();
                    for l in lm_core::i18n::Lang::all() {
                        if ui.radio(current == l.code(), l.name()).clicked() {
                            cmd = Some(Command::SetLanguage(l.code().into()));
                        }
                    }
                });
                ui.separator();
                if ui.button(tl!("Help")).clicked() {
                    help = true;
                }
                if ui.button(tl!("Credits")).clicked() {
                    cmd = Some(Command::ShowCredits);
                }
            });
        });
        if let Some(c) = cmd {
            self.shell.command(ctx, c);
        }
        if help {
            let folder = match self.shell.language().code() {
                "en" => String::new(),
                "pt" => "pt-br/".into(),
                c => format!("{c}/"),
            };
            if let Some(w) = web_sys::window() {
                let _ = w.open_with_url_and_target(&format!("https://nimblemap.com/{folder}help.html"), "_blank");
            }
        }
    }
}

/// The menu button's icon (Phosphor's list, in the font the shell installs).
fn egui_phosphor_list() -> &'static str {
    "\u{E2F0}"
}

// ---------------------------------------------------------------------------------------------
// Prefs

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn prefs_pairs(p: &Prefs) -> Vec<(&'static str, String)> {
    let mut v = vec![
        (
            "lm.theme",
            match p.theme {
                ThemePref::System => "system",
                ThemePref::Light => "light",
                ThemePref::Dark => "dark",
            }
            .to_string(),
        ),
        ("lm.units", if p.units == Units::Imperial { "imperial" } else { "metric" }.to_string()),
        ("lm.basemap.v2", p.basemap.clone()),
        ("lm.strength", p.strength.to_string()),
        ("lm.language", p.language.clone()),
    ];
    if let Some((lon, lat, z)) = p.view {
        v.push(("lm.view", format!("{lon:.5},{lat:.5},{z:.2}")));
    }
    v
}

fn prefs_text(p: &Prefs) -> String {
    prefs_pairs(p).into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(";")
}

fn parse_view(v: &str) -> Option<(f64, f64, f64)> {
    let n: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    match n.as_slice() {
        [lon, lat, z] if lon.is_finite() && lat.is_finite() && z.is_finite() => Some((*lon, *lat, *z)),
        _ => None,
    }
}

fn load_prefs() -> Prefs {
    let mut p = Prefs::default();
    let Some(s) = storage() else { return p };
    let get = |k: &str| s.get_item(k).ok().flatten();
    p.theme = match get("lm.theme").as_deref() {
        Some("light") => ThemePref::Light,
        Some("dark") => ThemePref::Dark,
        _ => ThemePref::System,
    };
    p.units = if get("lm.units").as_deref() == Some("imperial") { Units::Imperial } else { Units::Metric };
    if let Some(b) = get("lm.basemap.v2").filter(|b| lm_core::basemap::providers().iter().any(|p| p.id == b)) {
        p.basemap = b;
    }
    if let Some(v) = get("lm.strength").and_then(|v| v.parse::<f32>().ok()) {
        p.strength = v.clamp(0.4, 1.0);
    }
    if let Some(l) = get("lm.language").filter(|l| l == lm_core::i18n::AUTO || lm_core::i18n::Lang::from_code(l).is_some()) {
        p.language = l;
    }
    p.view = get("lm.view").and_then(|v| parse_view(&v));
    p
}

/// The browser's preferred languages, most preferred first.
fn browser_languages() -> Vec<String> {
    let Some(nav) = web_sys::window().map(|w| w.navigator()) else { return Vec::new() };
    let mut v: Vec<String> = nav.languages().iter().filter_map(|l| l.as_string()).collect();
    if v.is_empty()
        && let Some(l) = nav.language()
    {
        v.push(l);
    }
    v
}

fn media(q: &str) -> bool {
    web_sys::window().and_then(|w| w.match_media(q).ok().flatten()).is_some_and(|m| m.matches())
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// A query parameter's value, percent-decoded.
fn param(q: &str, key: &str) -> Option<String> {
    let raw = q.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))?;
    js_sys::decode_uri_component(&raw.replace('+', " ")).ok().and_then(|s| s.as_string())
}

// ---------------------------------------------------------------------------------------------
// Downloads to the user

fn save(name: &str, bytes: &[u8]) {
    if let Err(e) = download(name, bytes) {
        log::error!("download of {name} failed: {e}");
    }
}

/// Saves bytes through a temporary `<a download>` link.
fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let props = web_sys::BlobPropertyBag::new();
    props.set_type(if name.ends_with(".png") { "image/png" } else { "application/octet-stream" });
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &props).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(err)?.dyn_into().map_err(|_| "not an anchor".to_string())?;
    a.set_href(&url);
    a.set_download(name);
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(err)?;
    a.click();
    a.remove();
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    let window = web_sys::window().ok_or("no window")?;
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(err)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Downloads from the network

/// Protomaps' daily builds answer no cross-origin requests, so the page reads them through its
/// own site (`basemap/…`: a Cloudflare Pages Function in production, the preview server locally;
/// docs/lm/web-app.md › Hosting).
const BUILDS_PROXY: &str = "basemap/";

/// The address the page fetches for `url`: a daily build through the same-origin proxy.
fn proxied(url: &str) -> String {
    match url.strip_prefix(lm_mapview_archive::BUILDS) {
        Some(rest) => format!("{BUILDS_PROXY}{rest}"),
        None => url.to_string(),
    }
}

use lm_ui::mapview::archive as lm_mapview_archive;

/// Finds the newest daily build that answers (they are deleted after about a week).
fn find_basemap_build(ctx: egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        if !lm_net::allowed(lm_net::Purpose::Basemap) {
            lm_mapview_archive::set_found(None);
            return;
        }
        let metadata = fetch(&format!("{BUILDS_PROXY}builds.json"), None).await.ok().and_then(|b| String::from_utf8(b).ok());
        let today = (js_sys::Date::now() / 86_400_000.0).floor() as i64;
        let mut found = None;
        for url in lm_mapview_archive::candidates(metadata.as_deref(), today) {
            if fetch(&proxied(&url), Some("bytes=0-15")).await.is_ok() {
                found = Some(url);
                break;
            }
        }
        match &found {
            Some(u) => log::info!("basemap archive: {u}"),
            None => log::warn!("basemap archive: no daily build answered"),
        }
        lm_mapview_archive::set_found(found);
        ctx.request_repaint();
    });
}

/// A tile, part of a PMTiles archive (`…#range=a-b`, see `rustgis_tiles::cache::split_range`) or
/// an image, through the network gate: from the Cache API copy when there is one, else fetched
/// and copied there.
async fn fetch_tile(url: &str) -> Result<Vec<u8>, String> {
    lm_net::check(lm_net::Purpose::Basemap).map_err(|e| e.to_string())?;
    use rustgis_tiles::cache;
    let (url, _) = cache::split_download(url);
    let (url, tail) = cache::split_tail(url);
    let (url, range) = cache::split_range(url);
    let asked = match (range, tail) {
        (Some((a, b)), _) => Some(format!("bytes={a}-{b}")),
        (None, Some(n)) => Some(format!("bytes=-{n}")),
        (None, None) => None,
    };
    // The cache's key: the address with the range, as a query no server sees.
    let key = match &asked {
        Some(r) => format!("{url}{}lm-range={}", if url.contains('?') { '&' } else { '?' }, r.trim_start_matches("bytes=")),
        None => url.to_string(),
    };
    if let Some(bytes) = cache_get(&key).await {
        return Ok(bytes);
    }
    let bytes = fetch(&proxied(url), asked.as_deref()).await?;
    if let Some((a, b)) = range
        && bytes.len() as u64 > b - a + 1
    {
        // A range asked for and the whole file answered: only fine for a small archive read
        // from its start.
        if a > 0 {
            return Err(format!("{url}: {}", cache::NO_RANGES));
        }
    }
    if bytes.len() <= TILE_CACHE_MAX {
        let (key, copy) = (key.clone(), bytes.clone());
        wasm_bindgen_futures::spawn_local(async move { cache_put(&key, &copy).await });
    }
    Ok(bytes)
}

async fn tile_cache() -> Option<web_sys::Cache> {
    let caches = web_sys::window()?.caches().ok()?;
    let c = wasm_bindgen_futures::JsFuture::from(caches.open(TILE_CACHE)).await.ok()?;
    c.dyn_into().ok()
}

async fn cache_get(key: &str) -> Option<Vec<u8>> {
    let cache = tile_cache().await?;
    let hit = wasm_bindgen_futures::JsFuture::from(cache.match_with_str(key)).await.ok()?;
    let resp: web_sys::Response = hit.dyn_into().ok()?;
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().ok()?).await.ok()?;
    Some(js_sys::Uint8Array::new(&buf).to_vec())
}

async fn cache_put(key: &str, bytes: &[u8]) {
    let Some(cache) = tile_cache().await else { return };
    let mut copy = bytes.to_vec();
    let Ok(resp) = web_sys::Response::new_with_opt_u8_array(Some(&mut copy)) else { return };
    let _ = wasm_bindgen_futures::JsFuture::from(cache.put_with_str(key, &resp)).await;
}

/// Downloads `url` (the byte range `range` when given), at most 256 MB.
async fn fetch(url: &str, range: Option<&str>) -> Result<Vec<u8>, String> {
    const MAX: u64 = 256 << 20;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let promise = match range {
        Some(r) => {
            let init = web_sys::RequestInit::new();
            let headers = web_sys::Headers::new().map_err(err)?;
            headers.set("Range", r).map_err(err)?;
            init.set_headers(&headers);
            window.fetch_with_str_and_init(url, &init)
        }
        None => window.fetch_with_str(url),
    };
    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| {
            let why = e.dyn_ref::<js_sys::Error>().map(|e| String::from(e.message())).unwrap_or_else(|| err(e));
            format!("the browser couldn't reach it (offline, or blocked): {why}")
        })?
        .dyn_into()
        .map_err(err)?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    if let Ok(Some(len)) = resp.headers().get("content-length").map(|v| v.and_then(|s| s.parse::<u64>().ok()))
        && len > MAX
    {
        return Err(format!("{len} bytes is too large"));
    }
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().map_err(err)?).await.map_err(err)?;
    let bytes = js_sys::Uint8Array::new(&buf);
    if u64::from(bytes.length()) > MAX {
        return Err("too large".into());
    }
    Ok(bytes.to_vec())
}
