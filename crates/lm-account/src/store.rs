//! Where the session record lives: the macOS Keychain (Security.framework) or the Windows
//! Credential Manager, or memory (every test: none writes to a real keychain or raises a
//! permission dialog).
//!
//! The record (`Session::to_record`) is hex. On macOS it is passed to Security.framework as bytes
//! (never on a command line). On Windows it still goes to PowerShell on standard input.

use std::sync::Mutex;

/// The keychain service / Credential Manager resource. An internal identifier, like the bundle
/// id: not display text (lm_core::brand).
pub const SERVICE: &str = "app.layeredmap.pro.account";
pub const ACCOUNT: &str = "session";

/// A place for one secret.
pub trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<String>, String>;
    fn save(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// In memory, for tests and for builds without a system store.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl MemoryStore {
    pub fn with(secret: &str) -> MemoryStore {
        MemoryStore(Mutex::new(Some(secret.to_string())))
    }
}

impl SecretStore for MemoryStore {
    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.0.lock().map_err(|_| "store poisoned")?.clone())
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        *self.0.lock().map_err(|_| "store poisoned")? = Some(secret.to_string());
        Ok(())
    }

    fn delete(&self) -> Result<(), String> {
        *self.0.lock().map_err(|_| "store poisoned")? = None;
        Ok(())
    }
}

/// Only hex reaches the system tools (the record is hex by construction; this is the guard).
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
fn check_hex(secret: &str) -> Result<(), String> {
    if !secret.is_empty() && secret.bytes().all(|b| b.is_ascii_hexdigit()) { Ok(()) } else { Err("not a session record".into()) }
}

/// The system's store: the Keychain on macOS, the Credential Manager on Windows, none elsewhere.
#[cfg(target_os = "macos")]
pub fn system() -> Option<std::sync::Arc<dyn SecretStore>> {
    Some(std::sync::Arc::new(Keychain))
}

#[cfg(windows)]
pub fn system() -> Option<std::sync::Arc<dyn SecretStore>> {
    Some(std::sync::Arc::new(CredentialManager))
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn system() -> Option<std::sync::Arc<dyn SecretStore>> {
    None
}

/// The login keychain, through Security.framework (`security-framework` crate; no `unsafe` in
/// this crate). Same service and account as before, so existing items remain readable.
#[cfg(target_os = "macos")]
pub struct Keychain;

#[cfg(target_os = "macos")]
impl SecretStore for Keychain {
    fn load(&self) -> Result<Option<String>, String> {
        use security_framework::passwords::get_generic_password;
        use security_framework_sys::base::errSecItemNotFound;
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).map_err(|_| "unreadable keychain item")?;
                Ok((!s.is_empty()).then_some(s))
            }
            Err(e) if e.code() == errSecItemNotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        use security_framework::passwords::set_generic_password;
        check_hex(secret)?;
        set_generic_password(SERVICE, ACCOUNT, secret.as_bytes()).map_err(|e| e.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        use security_framework::passwords::delete_generic_password;
        use security_framework_sys::base::errSecItemNotFound;
        match delete_generic_password(SERVICE, ACCOUNT) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == errSecItemNotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// The Credential Manager (Web Credentials), through Windows PowerShell's access to the WinRT
/// `PasswordVault`: a safe route, as the app keeps unsafe Win32 calls out of everything but its
/// own Windows module (docs/lm/windows.md).
#[cfg(windows)]
pub struct CredentialManager;

#[cfg(windows)]
const VAULT: &str = "$ErrorActionPreference='Stop';\
    [void][Windows.Security.Credentials.PasswordVault,Windows.Security.Credentials,ContentType=WindowsRuntime];\
    $v=New-Object Windows.Security.Credentials.PasswordVault;";

#[cfg(windows)]
fn powershell(script: &str, input: &str) -> Result<std::process::Output, String> {
    let full = format!("{VAULT}{script}");
    run_with_input("powershell.exe", &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &full], input)
}

#[cfg(windows)]
impl SecretStore for CredentialManager {
    fn load(&self) -> Result<Option<String>, String> {
        let script = format!(
            "try {{ $c=$v.Retrieve('{SERVICE}','{ACCOUNT}') }} catch {{ exit 3 }}; $c.RetrievePassword(); [Console]::Out.Write($c.Password)"
        );
        let out = powershell(&script, "")?;
        if out.status.code() == Some(3) {
            return Ok(None);
        }
        let s = String::from_utf8(out.stdout).map_err(|_| "unreadable credential")?.trim().to_string();
        if out.status.success() { Ok((!s.is_empty()).then_some(s)) } else { Err("the Credential Manager couldn't be read".into()) }
    }

    fn save(&self, secret: &str) -> Result<(), String> {
        check_hex(secret)?;
        let script = format!(
            "$s=[Console]::In.ReadLine(); try {{ $v.Remove($v.Retrieve('{SERVICE}','{ACCOUNT}')) }} catch {{}}; \
             $v.Add((New-Object Windows.Security.Credentials.PasswordCredential('{SERVICE}','{ACCOUNT}',$s)))"
        );
        let out = powershell(&script, &format!("{secret}\n"))?;
        if out.status.success() { Ok(()) } else { Err("the Credential Manager refused the item".into()) }
    }

    fn delete(&self) -> Result<(), String> {
        let script = format!("try {{ $v.Remove($v.Retrieve('{SERVICE}','{ACCOUNT}')) }} catch {{}}");
        let out = powershell(&script, "")?;
        if out.status.success() { Ok(()) } else { Err("the Credential Manager item couldn't be deleted".into()) }
    }
}

/// Runs a program with `input` on its standard input (no console window on Windows).
#[cfg(windows)]
fn run_with_input(program: &str, args: &[&str], input: &str) -> Result<std::process::Output, String> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    }
    child.wait_with_output().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_store_keeps_one_secret() {
        let s = MemoryStore::default();
        assert_eq!(s.load().unwrap(), None);
        s.save("ab12").unwrap();
        s.save("cd34").unwrap();
        assert_eq!(s.load().unwrap().as_deref(), Some("cd34"));
        s.delete().unwrap();
        assert_eq!(s.load().unwrap(), None);
        assert_eq!(MemoryStore::with("ff").load().unwrap().as_deref(), Some("ff"));
    }

    #[test]
    fn only_hex_goes_to_the_system_tools() {
        assert!(check_hex("00aaFF").is_ok());
        for bad in ["", "abc xyz", "ab\n-w other", "ab'c", "ab;rm"] {
            assert!(check_hex(bad).is_err(), "{bad:?}");
        }
    }
}
