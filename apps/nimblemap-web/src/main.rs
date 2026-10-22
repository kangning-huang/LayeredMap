//! NimbleMap in the browser (docs/lm/web-app.md).
//!
//! Runs the same `lm_ui::Shell` as the desktop app through eframe's web runner (WebGPU where
//! available, WebGL2 otherwise). Build with `apps/nimblemap-web/build.sh` (static files in a
//! `dist` folder).
//!
//! Differences from the desktop app: files come from the browser's file picker or a drop and are
//! held in memory (`lm_jobs::vfs`), never uploaded; the examples are fetched from the site on
//! first use; exports download; background work runs between frames on the page's one thread
//! (`lm_core::task`); there is no menu bar, update check, voice input or saving to a path.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

// Pure logic, tested natively; the browser build uses it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod files;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("nimblemap-web only runs in the browser: build it with apps/nimblemap-web/build.sh");
}
