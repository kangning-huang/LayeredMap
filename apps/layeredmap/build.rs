//! On Windows (WP6.2), embeds the app icon (`packaging/lm/windows/layeredmap.ico`, from
//! `tools/lm/make_app_icon.py`), version information and the application manifest (per-monitor
//! DPI awareness, long paths) in `layeredmap.exe`. Nothing happens for other targets.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?).join("../../packaging/lm/windows");
    let (ico, manifest) = (dir.join("layeredmap.ico"), dir.join("layeredmap.exe.manifest"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", ico.display());
    println!("cargo:rerun-if-changed={}", manifest.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let version = std::env::var("CARGO_PKG_VERSION")?;
    // VERSIONINFO wants four numbers: drop a pre-release suffix (0.2.0-rc.1 -> 0,2,0,0); the
    // fourth is the CI build number when there is one.
    let nums: Vec<&str> = version.split('-').next().unwrap_or("0.0.0").split('.').collect();
    let num = |i: usize| nums.get(i).copied().unwrap_or("0");
    println!("cargo:rerun-if-env-changed=LM_BUILD_NUMBER");
    let build: u16 = std::env::var("LM_BUILD_NUMBER").ok().and_then(|b| b.parse().ok()).unwrap_or(0);
    let file_version = format!("{},{},{},{build}", num(0), num(1), num(2));
    // Resource scripts take C strings: backslashes doubled.
    let path = |p: &PathBuf| p.display().to_string().replace('\\', "\\\\");
    let rc = PathBuf::from(std::env::var("OUT_DIR")?).join("layeredmap.rc");
    std::fs::write(
        &rc,
        format!(
            r#"#pragma code_page(65001)
1 ICON "{ico}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {file_version}
PRODUCTVERSION {file_version}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "LayeredMap"
      VALUE "FileDescription", "LayeredMap"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "layeredmap"
      VALUE "LegalCopyright", "Copyright the LayeredMap and RustGIS contributors. Apache-2.0."
      VALUE "OriginalFilename", "layeredmap.exe"
      VALUE "ProductName", "LayeredMap"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
            ico = path(&ico),
            manifest = path(&manifest),
        ),
    )?;
    // Without a resource compiler (rc.exe, llvm-rc or windres) the exe has no icon or manifest:
    // warn rather than fail a plain `cargo build`. The installer build (CI) has rc.exe.
    if let Err(e) = embed_resource::compile(&rc, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=layeredmap.exe gets no icon or manifest: {e}");
    }
    Ok(())
}
