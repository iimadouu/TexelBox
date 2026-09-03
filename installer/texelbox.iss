; TexelBox — Inno Setup 6 script (spec §9 Phase 13 packaging).
; Compile via scripts\release.ps1 (passes /DAppVersion + /DPlan) or manually:
;   ISCC /DAppVersion=0.1.0 /DPlan=pro installer\texelbox.iss
;
; The app is a single native binary: locales, UI and the entitlement gate
; are all embedded (spec §8 / §4), so the payload is just texelbox.exe.
; User data (presets, license cache, device id) lives in %APPDATA%\TexelBox
; and is deliberately NOT removed on uninstall.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef Plan
  #define Plan "free"
#endif

#if Plan = "pro"
  #define PlanLabel "Pro"
  #define PlanDir "TexelBox-pro"
  #define PlanOutput "TexelBox-{#AppVersion}-Pro-Setup"
#elif Plan = "trial"
  #define PlanLabel "Trial"
  #define PlanDir "TexelBox-trial"
  #define PlanOutput "TexelBox-{#AppVersion}-Trial-Setup"
#else
  #define PlanLabel "Free"
  #define PlanDir "TexelBox-free"
  #define PlanOutput "TexelBox-{#AppVersion}-Free-Setup"
#endif

[Setup]
AppId={{9C2F4A6E-TEXB-4BOX-8D31-2026A0000001}
AppName=TexelBox {#PlanLabel}
AppVersion={#AppVersion}
AppPublisher=TexelBox
AppPublisherURL=https://texelbox.app
AppSupportURL=https://texelbox.app/support
SetupIconFile=..\crates\tbx-app\assets\icon.ico
DefaultDirName={autopf}\TexelBox{#PlanLabel}
DefaultGroupName=TexelBox {#PlanLabel}
UninstallDisplayIcon={app}\texelbox.exe
OutputBaseFilename={#PlanOutput}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Native binary is x64-only.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; SmartScreen-friendly: signed releases set these automatically; unsigned
; dev builds will show the "unknown publisher" warning until signed (§4.5).
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
; Small quality-of-life: no desktop icon by default, task is opt-in.

[Languages]
; Installer UI languages match the product locales (spec §8).
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\dist\{#PlanDir}\texelbox.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\crates\tbx-app\assets\icon.ico"; DestDir: "{app}"; Flags: dontcopy

[Icons]
Name: "{group}\TexelBox {#PlanLabel}"; Filename: "{app}\texelbox.exe"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uninstall TexelBox {#PlanLabel}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\TexelBox {#PlanLabel}"; Filename: "{app}\texelbox.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\texelbox.exe"; Description: "{cm:LaunchProgram,TexelBox {#PlanLabel}}"; Flags: nowait postinstall skipifsilent
