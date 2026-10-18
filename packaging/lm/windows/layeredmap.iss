; LayeredMap for Windows (WP6.2): the Inno Setup 6 script for the Free and Pro installers.
; Built by packaging/lm/windows/build-installer.ps1, which stages the files and passes:
;   /DTier=free|pro  /DAppVersion=0.2.0  /DBuildNumber=0  /DStage=<staged folder>  /DOutDir=<dir>
;   /DSign=1 and /Slmsign=<signtool command> when a signing certificate is configured
; Free and Pro install side by side: each has its own AppId, folder, Start menu entry, executable
; name and file-type ProgIDs. Installs for the current user without administrator rights (school
; laptops); someone with rights can choose "all users" in the first page.

#ifndef Tier
  #define Tier "free"
#endif
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef BuildNumber
  #define BuildNumber "0"
#endif
#ifndef Stage
  #error Pass /DStage=<folder staged by build-installer.ps1>
#endif
#ifndef OutDir
  #define OutDir "."
#endif

#if Tier == "pro"
  #define AppName "LayeredMap Pro"
  #define AppExe "layeredmap-pro.exe"
  ; Never change an AppId: Windows tells installs apart by it.
  #define AppGuid "{{3E0B7A52-6C1D-4E8A-9F3B-2D7C5A9E1B44}"
  #define ProgPrefix "LayeredMapPro"
  #define OutName "LayeredMap-Pro-" + AppVersion + "-" + BuildNumber + "-windows-x64-setup"
#else
  #define AppName "LayeredMap"
  #define AppExe "layeredmap.exe"
  #define AppGuid "{{8C4F2E91-1B7A-4D3C-A6E5-0F9D8B2C7A13}"
  #define ProgPrefix "LayeredMap"
  #define OutName "LayeredMap-" + AppVersion + "-" + BuildNumber + "-windows-x64-setup"
#endif

[Setup]
AppId={#AppGuid}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=LayeredMap
AppPublisherURL=https://github.com/opengeos/RustGIS
VersionInfoVersion={#AppVersion}.{#BuildNumber}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Per-user by default (%LOCALAPPDATA%\Programs); "all users" needs administrator rights.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
WizardStyle=modern
SetupIconFile=layeredmap.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
LicenseFile={#Stage}\Resources\LICENSE
OutputDir={#OutDir}
OutputBaseFilename={#OutName}
Compression=lzma2/max
SolidCompression=yes
; File associations change: tell Explorer to refresh its icons and Open With lists.
ChangesAssociations=yes
CloseApplications=yes
#ifdef Sign
; `lmsign` is defined on the ISCC command line (/Slmsign=...) by build-installer.ps1 from the
; LM_SIGNTOOL_ARGS environment variable; the installer and its uninstaller are both signed.
SignTool=lmsign
SignedUninstaller=yes
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
; Chinese Simplified is an official translation from Inno Setup 6.5.0 on.
#if FileExists(CompilerPath + "Languages\ChineseSimplified.isl")
Name: "zh"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The executable (already signed by build-installer.ps1 when signing is configured), then the
; resources the app finds beside it: examples, the tier, licences, and for Pro the built-in
; reference layers.
Source: "{#Stage}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\Resources\*"; DestDir: "{app}\Resources"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; HKA is HKCU for a per-user install and HKLM for an all-users one.
; .lmap is LayeredMap's own type: opened for editing, with the app's icon. With both tiers
; installed, the one installed last is the default; the other stays in Open With.
Root: HKA; Subkey: "Software\Classes\.lmap"; ValueType: string; ValueName: ""; ValueData: "{#ProgPrefix}.Map"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.lmap"; ValueType: string; ValueName: "Content Type"; ValueData: "application/vnd.layeredmap.map+zip"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.lmap\OpenWithProgids"; ValueType: string; ValueName: "{#ProgPrefix}.Map"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Map"; ValueType: string; ValueName: ""; ValueData: "LayeredMap map"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Map\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Map\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
; Data files: offered in Open With, never made the default (as LSHandlerRank Alternate on macOS).
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Data"; ValueType: string; ValueName: ""; ValueData: "Map data"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Data"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "Map data"
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Data\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"
Root: HKA; Subkey: "Software\Classes\{#ProgPrefix}.Data\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
#define DataExt(str Ext) \
  'Root: HKA; Subkey: "Software\Classes\' + Ext + '\OpenWithProgids"; ValueType: string; ValueName: "' + ProgPrefix + '.Data"; ValueData: ""; Flags: uninsdeletevalue' + NewLine + \
  'Root: HKA; Subkey: "Software\Classes\Applications\' + AppExe + '\SupportedTypes"; ValueType: string; ValueName: "' + Ext + '"; ValueData: ""'
#emit DataExt(".geojson")
#emit DataExt(".shp")
#emit DataExt(".gpkg")
#emit DataExt(".kml")
#emit DataExt(".kmz")
#emit DataExt(".gpx")
#emit DataExt(".nc")
#emit DataExt(".nc4")
#emit DataExt(".cdf")
#emit DataExt(".tif")
#emit DataExt(".tiff")
#emit DataExt(".csv")
#emit DataExt(".tsv")
#emit DataExt(".xlsx")
#emit DataExt(".lmap")
; The app's entry in Open With's "Choose another app" list.
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
