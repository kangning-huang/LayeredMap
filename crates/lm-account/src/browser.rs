//! Opens the sign-in page in the system browser (RFC 8252: never an embedded web view, so the
//! app never sees a password).

/// Opens `url` in the default browser. Only https addresses (and this computer's, for tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn open(url: &str) -> Result<(), String> {
    if !crate::config::allowed_url(url) {
        return Err(format!("{url}: not opened (https only)"));
    }
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("/usr/bin/open");
        c.arg(url);
        c
    };
    // rundll32's URL handler takes the address as one argument, with no shell to split it at `&`.
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    match cmd.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("the browser couldn't be opened ({s})")),
        Err(e) => Err(format!("the browser couldn't be opened: {e}")),
    }
}
