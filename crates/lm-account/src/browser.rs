//! Opens the sign-in page in the system browser (RFC 8252: never an embedded web view, so the
//! app never sees a password).

/// Opens `url` in the default browser. Only https addresses (and this computer's, for tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn open(url: &str) -> Result<(), String> {
    if !crate::config::allowed_url(url) {
        return Err(format!("{url}: not opened (https only)"));
    }
    #[cfg(target_os = "macos")]
    {
        open_macos(url)
    }
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("rundll32.exe");
        // rundll32's URL handler takes the address as one argument, with no shell to split it at `&`.
        cmd.args(["url.dll,FileProtocolHandler", url]);
        match cmd.status() {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!("the browser couldn't be opened ({s})")),
            Err(e) => Err(format!("the browser couldn't be opened: {e}")),
        }
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(url);
        match cmd.status() {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!("the browser couldn't be opened ({s})")),
            Err(e) => Err(format!("the browser couldn't be opened: {e}")),
        }
    }
}

/// NSWorkspace.openURL — no `/usr/bin/open`, so a sandboxed build can open the sign-in page.
#[cfg(target_os = "macos")]
fn open_macos(url: &str) -> Result<(), String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};
    let Some(nsurl) = NSURL::URLWithString(&NSString::from_str(url)) else {
        return Err(format!("{url}: not a URL"));
    };
    if NSWorkspace::sharedWorkspace().openURL(&nsurl) { Ok(()) } else { Err("the browser couldn't be opened".into()) }
}
