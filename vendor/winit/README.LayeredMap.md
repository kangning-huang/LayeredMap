Patched winit 0.30.13 for Mac App Store builds.

Upstream links the private `_CGSSetWindowBackgroundBlurRadius` symbol
(guideline 2.5.1). This copy removes that FFI and makes `Window::set_blur`
a no-op on macOS. NimbleMap never enables blur.

Synced from crates.io winit 0.30.13. See Cargo.toml `[patch.crates-io]`.
