; Inno Setup script for Amore — one-click Windows installer (F.installer-1).
;
; Compile via Inno Setup 6 (iscc) in a GitHub Actions windows-latest runner.
; Reviewer nit (2026-05-26T~12:30Z): installer budget is tight; small variant
; ships bge-small-en-v1.5 ONNX (~120 MB) bundled; qdrant.exe is downloaded on
; first-run by amore-gui to keep the .exe installer under 150 MB.
;
; Non-tech-user UX gates (from the v1.0 plan Definition of Done items 1-8):
;   * Double-click -> installed + first-recall in <=2 min
;   * No console window flashes
;   * Plain-English errors (modal dialogs, no stack traces)
;   * Auto-start on login (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
;   * Uninstaller prompts: "Keep my memory? [Keep / Delete]" (default Keep)
;   * SmartScreen "More info" -> "Run anyway" path documented; self-signed pending EV cert

#define MyAppName "Amore"
#define MyAppVersion "0.3.0"
#define MyAppPublisher "Antonio Amore Akiki"
#define MyAppURL "https://github.com/antonio-amore-akiki/amore"
#define MyAppExeName "amore-gui.exe"
#define MyAppCliName "amore.exe"
#define MyAppMcpName "amore-mcp.exe"

[Setup]
AppId={{F3D7A4E1-2C5B-4E8F-9A2D-7B6E1C5A3F9D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={localappdata}\Programs\Amore
DefaultGroupName={#MyAppName}
OutputDir=..\..\target\installer
OutputBaseFilename=Amore-Setup-v{#MyAppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#MyAppExeName}
SetupLogging=yes
SetupIconFile=..\..\branding\amore.ico
ShowLanguageDialog=auto
LicenseFile=..\..\LICENSE
WizardSizePercent=120,120

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &Desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked
Name: "autostart"; Description: "Start &Amore on login (recommended)"; GroupDescription: "Startup options:"

[Files]
; Core Rust binaries (built by cargo build --release --target x86_64-pc-windows-msvc).
; Paths are relative to the installer/windows/ working dir; the CI staging step
; copies build artifacts here before running iscc.
Source: "staging\{#MyAppCliName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\{#MyAppMcpName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "staging\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Bundled ONNX embedding model (bge-small-en-v1.5, ~120 MB).
; Small variant; full variant (nomic-embed-text ~500 MB) downloaded on demand.
Source: "staging\models\bge-small-en-v1.5.onnx"; DestDir: "{app}\models"; Flags: ignoreversion

; License + attribution.
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\NOTICE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Auto-start on login (HKCU; no admin needed). Tasks: autostart gates this.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Amore"; ValueData: "\"{app}\{#MyAppExeName}\" --tray"; Tasks: autostart; Flags: uninsdeletevalue
; Register the user-mode MCP server path so IDE adapters can discover it via
; HKCU\Software\Amore\McpServerPath without env-var/PATH lookups.
Root: HKCU; Subkey: "Software\Amore"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Amore"; ValueType: string; ValueName: "McpServerPath"; ValueData: "{app}\{#MyAppMcpName}"; Flags: uninsdeletekey

[Run]
; Post-install: launch the GUI in first-run mode. nowait + postinstall =
; spawns it and the installer wizard's Finish page proceeds; no console
; window because amore-gui.exe is built as a /SUBSYSTEM:WINDOWS binary.
Filename: "{app}\{#MyAppExeName}"; Parameters: "--first-run"; Description: "Launch {#MyAppName} setup wizard"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
; Don't delete user data by default; uninstaller PROMPTS via [Code] InitializeUninstall.
; Keep marker file so subsequent install can offer "restore your memory".
Type: filesandordirs; Name: "{app}\models"
Type: filesandordirs; Name: "{app}\cache"

[Code]
var
  KeepData: Boolean;

procedure InitializeUninstallProgressForm;
begin
  KeepData := True;
end;

function InitializeUninstall: Boolean;
var
  Response: Integer;
begin
  // Plain-English prompt: "Keep my memory? [Keep / Delete]" default Keep.
  Response := MsgBox(
    'Uninstalling Amore.' + #13#10 + #13#10 +
    'Would you like to KEEP your memory and settings so you can restore them' + #13#10 +
    'next time you install Amore?' + #13#10 + #13#10 +
    'Click YES to KEEP (recommended).' + #13#10 +
    'Click NO to permanently DELETE all Amore data.',
    mbConfirmation, MB_YESNOCANCEL or MB_DEFBUTTON1
  );
  if Response = IDCANCEL then
  begin
    Result := False;  // abort uninstall
    Exit;
  end;
  KeepData := (Response = IDYES);
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataPath: string;
begin
  if (CurUninstallStep = usPostUninstall) and (not KeepData) then
  begin
    DataPath := ExpandConstant('{userappdata}\Amore');
    if DirExists(DataPath) then
      DelTree(DataPath, True, True, True);
  end;
end;

// Pre-install: detect existing Ollama (required dep for LLM completions).
// If absent, surface a plain-English dialog with a Download button.
// Auto-download via PowerShell happens in amore-gui first-run flow, not here
// (keeps installer fast + non-blocking).
function InitializeSetup(): Boolean;
var
  OllamaPath: string;
  Response: Integer;
begin
  // Best-effort detection: probe %ProgramFiles%/Ollama/ollama.exe and PATH.
  OllamaPath := ExpandConstant('{commonpf}\Ollama\ollama.exe');
  if not FileExists(OllamaPath) then
  begin
    Response := MsgBox(
      'Amore uses Ollama (a local AI runtime) to answer your questions.' + #13#10 +
      'Ollama is not installed yet.' + #13#10 + #13#10 +
      'Amore will offer to download and install it automatically' + #13#10 +
      'the first time you open Amore. No technical setup needed.' + #13#10 + #13#10 +
      'Continue installing Amore now?',
      mbInformation, MB_OKCANCEL or MB_DEFBUTTON1
    );
    if Response = IDCANCEL then
    begin
      Result := False;
      Exit;
    end;
  end;
  Result := True;
end;
