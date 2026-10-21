# Builds the NimbleMap installer for Windows (WP6.2), the counterpart of bundle-macos.sh and
# build-dmg.sh. Run on Windows with Rust (MSVC) and Inno Setup 6:
#
#   pwsh packaging/lm/windows/build-installer.ps1 [-OutDir dist\lm]
#
# Writes NimbleMap-<version>-<build>-windows-x64-setup.exe to OutDir. One app: Pro's features
# unlock when someone signs in (docs/lm/accounts-design.md › Entitlement).
#
# Signing runs only when LM_SIGNTOOL_ARGS is set: the arguments signtool needs after `sign` to
# reach the certificate, for example (docs/lm/signing.md):
#   Azure Artifact Signing: /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256
#                           /dlib <path>\Azure.CodeSigning.Dlib.dll /dmdf <path>\metadata.json
#   A certificate on a token or cloud HSM: /fd SHA256 /tr <timestamp url> /td SHA256 /sha1 <thumbprint>
# The executable, the installer and its uninstaller are signed and then verified. Without it the
# installer is unsigned: fine for testing, but SmartScreen warns on every other PC.
#
# Other environment variables, as on macOS: LM_BUILD_NUMBER, LM_UPDATE_FEED_URL (passed to cargo),
# LM_REFERENCE_DATA (the built-in reference layers), and LM_ACCOUNT_CONFIG (a path) or
# LM_ACCOUNT_CONFIG_JSON (its text) for the account config. No build carries an assistant key.
param([string]$OutDir = "")

$ErrorActionPreference = "Stop"

# The display name in the installer's file name; keep in step with lm_core::brand and the
# Brand define in layeredmap.iss (docs/lm/naming.md).
$brand = "NimbleMap"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
if (-not $OutDir) { $OutDir = Join-Path $root "dist\lm" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root "target" }
$version = (Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$build = if ($env:LM_BUILD_NUMBER) { $env:LM_BUILD_NUMBER } else { "0" }
$exeName = "layeredmap.exe"

cargo build --release --locked -p layeredmap --manifest-path (Join-Path $root "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

# The staged folder mirrors the install: the exe, and Resources beside it (lm_core::platform).
$stage = Join-Path $OutDir "stage-windows"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
$res = Join-Path $stage "Resources"
New-Item -ItemType Directory -Force -Path $res | Out-Null
Copy-Item (Join-Path $targetDir "release\layeredmap.exe") (Join-Path $stage $exeName)
# The Examples menu (WP2.9): Resources\examples. AppleDouble files from the exFAT drive are left out.
Copy-Item -Recurse (Join-Path $root "assets\examples") (Join-Path $res "examples")
Get-ChildItem -Recurse -Force -Path $res -Filter "._*" | Remove-Item -Force
# Licences (Gate 5 licence audit).
Copy-Item (Join-Path $root "LICENSE"), (Join-Path $root "ATTRIBUTION.md") $res
if (Test-Path (Join-Path $root "ATTRIBUTION-crates.md")) { Copy-Item (Join-Path $root "ATTRIBUTION-crates.md") $res }

# Built-in reference layers (tools/lm/reference/build_reference.py), as in bundle-macos.sh.
$ref = $env:LM_REFERENCE_DATA
if ($ref -and (Test-Path (Join-Path $ref "manifest.json"))) {
  $dst = Join-Path $res "reference"
  Copy-Item -Recurse $ref $dst
  if (Test-Path (Join-Path $dst "raw")) { Remove-Item -Recurse -Force (Join-Path $dst "raw") }
  Get-ChildItem -Recurse -Force -Path $dst -Filter "._*" | Remove-Item -Force
} else {
  Write-Warning "build-installer: no reference data at '$ref' (built-in layers left out)"
}
# The account config (docs/lm/accounts-design.md), as in bundle-macos.sh. Without it nobody can
# sign in, so Pro stays locked.
if ($env:LM_ACCOUNT_CONFIG_JSON) {
  Set-Content -Encoding utf8 -Path (Join-Path $res "account.json") -Value $env:LM_ACCOUNT_CONFIG_JSON
  Write-Host "build-installer: account config from LM_ACCOUNT_CONFIG_JSON"
} elseif ($env:LM_ACCOUNT_CONFIG) {
  Copy-Item $env:LM_ACCOUNT_CONFIG (Join-Path $res "account.json")
  Write-Host "build-installer: account config from $($env:LM_ACCOUNT_CONFIG)"
} else {
  Write-Warning "build-installer: no account config (LM_ACCOUNT_CONFIG); this build can't sign in, so Pro stays locked"
}

# signtool from the newest Windows SDK on the machine.
function Find-SignTool {
  $kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
  $found = Get-ChildItem -Path $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match "\\x64\\" } | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $found) { throw "signtool.exe not found (install the Windows SDK)" }
  return $found.FullName
}

$sign = [bool]$env:LM_SIGNTOOL_ARGS
if ($sign) {
  $signtool = Find-SignTool
  $signArgs = $env:LM_SIGNTOOL_ARGS
  & $signtool sign $signArgs.Split(" ", [StringSplitOptions]::RemoveEmptyEntries) (Join-Path $stage $exeName)
  if ($LASTEXITCODE -ne 0) { throw "signing the exe failed" }
  & $signtool verify /pa /v (Join-Path $stage $exeName)
  if ($LASTEXITCODE -ne 0) { throw "the exe's signature does not verify" }
} else {
  Write-Warning "build-installer: no LM_SIGNTOOL_ARGS; the installer is unsigned (SmartScreen will warn)"
}

# Inno Setup's compiler: ISCC on the PATH, or its default folder.
$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
  foreach ($p in @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 6\ISCC.exe", "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe")) {
    if (Test-Path $p) { $iscc = $p; break }
  }
}
if (-not $iscc) { throw "Inno Setup 6 (ISCC.exe) not found: winget install JRSoftware.InnoSetup" }

$isccArgs = @("/Qp", "/DAppVersion=$version", "/DBuildNumber=$build", "/DStage=$stage", "/DOutDir=$OutDir")
if ($sign) {
  # Inno runs this for the installer and the uninstaller; $f is the file, $q a quote.
  $isccArgs += "/DSign=1"
  $isccArgs += "/Slmsign=`$q$signtool`$q sign $($env:LM_SIGNTOOL_ARGS) `$f"
}
& $iscc @isccArgs (Join-Path $PSScriptRoot "layeredmap.iss")
if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }

$setup = Join-Path $OutDir "$brand-$version-$build-windows-x64-setup.exe"
if (-not (Test-Path $setup)) { throw "no installer at $setup" }
if ($sign) {
  & $signtool verify /pa /v $setup
  if ($LASTEXITCODE -ne 0) { throw "the installer's signature does not verify" }
}
$hash = (Get-FileHash -Algorithm SHA256 $setup).Hash.ToLower()
Write-Host "build-installer: $setup"
Write-Host "build-installer: sha256 $hash"
