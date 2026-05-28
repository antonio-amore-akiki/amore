; amore.iss — Inno Setup script that wraps amore-windows-x64.msi into a consumer-friendly .exe.
; Free toolchain (Inno Setup, ISC-equivalent license). User double-clicks the .exe; Inno's first-launch
; screen runs msiexec /i silently. Output: amore-windows-x64.exe at the same dir as ISCC was invoked.
;
; Build:  iscc /Q amore.iss                    — lite installer (~15 MB)
;         iscc /Q /DFatInstaller amore.iss      — fat installer (~535 MB, B3, F20)
; CI:     see .github/workflows/release.yml job windows-build (Inno via chocolatey).
;
; Fat installer (B3, F20): bundles ollama.exe + qdrant.exe + nomic-embed-text GGUF model.
; Realistic sizes: nomic-embed-text ~274 MB + qdrant.exe ~80 MB + ollama.exe ~150 MB + amore ~30 MB.
; GitHub Releases per-asset cap: 2 GB — fat installer fits.
; Default release page: fat for first-time users, lite for upgraders.
; If fat installer >700 MB: build with /DBundleModel=false to skip model (post-install Ollama pull).

#define AmoreVersion "1.0.0"
; Fat-installer: override OutputBaseFilename when /DFatInstaller is set.
#ifdef FatInstaller
  #define OutputBase "amore-windows-x64-fat"
#else
  #define OutputBase "amore-windows-x64"
#endif

[Setup]
AppId={{6F2B3A1E-AAA0-4D5A-9BB1-AMORE10220250527}}
AppName=Amore
AppVersion={#AmoreVersion}
AppPublisher=Antonio
AppPublisherURL=https://github.com/antonio-amore-akiki/amore
AppSupportURL=https://github.com/antonio-amore-akiki/amore/issues
DefaultDirName={autopf}\Amore
DisableProgramGroupPage=yes
OutputBaseFilename={#OutputBase}
OutputDir=.
Compression=lzma2/ultra
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayName=Amore {#AmoreVersion}
UninstallDisplayIcon={app}\amore-gui.exe

[Languages]
; Five required languages per D1 user requirement 2026-05-28 (Lebanon + EU: EN/FR/NL/DE/AR).
; Arabic uses a vendored .isl from the Unofficial Inno Setup Translations project
; (https://github.com/jrsoftware/issrc/blob/main/Files/Languages).
; TODO: vendor/Arabic.isl is a stub — replace with the real Arabic.isl before the next
; Windows build. Obtain from https://github.com/jrsoftware/unofficial-inno-setup-translations
; and pin its SHA256 in this comment once vendored.
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "nl"; MessagesFile: "compiler:Languages\Dutch.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "ar"; MessagesFile: "vendor\Arabic.isl"

[Files]
; The bundled MSI is extracted to {tmp} and removed after install completes.
; CI passes -DMsiPath=...; locally you can place amore-windows-x64.msi next to amore.iss.
#ifdef MsiPath
Source: "{#MsiPath}"; DestDir: "{tmp}"; DestName: "amore-windows-x64.msi"; Flags: deleteafterinstall
#else
Source: "amore-windows-x64.msi"; DestDir: "{tmp}"; Flags: deleteafterinstall
#endif

; --- FAT INSTALLER EXTRA FILES (B3, F20) ---
; Only included when compiling with /DFatInstaller.
; Provides air-gapped / first-run-offline install capability.
; Expected staging layout (built by build-installer-windows.ps1 -BundleDeps):
;   staging\fat\ollama.exe           (~150 MB, ollama v0.24.0)
;   staging\fat\qdrant.exe           (~80 MB, qdrant v1.18.1)
;   staging\fat\models\nomic-embed-text.gguf  (~274 MB, default embedding model)
;   staging\fat\cosign-verify-mini.exe        (~3 MB, B4 pre-extract verifier — see [Code])
;
; skipifsourcedoesntexist: local iscc validation succeeds without staging; CI populates staging.
#ifdef FatInstaller
Source: "staging\fat\ollama.exe"; DestDir: "{app}"; DestName: "ollama.exe"; Flags: ignoreversion skipifsourcedoesntexist
Source: "staging\fat\qdrant.exe"; DestDir: "{app}"; DestName: "qdrant.exe"; Flags: ignoreversion skipifsourcedoesntexist
; BundleModel=false: skip the GGUF file — post-install amore-gui pulls it via `ollama pull nomic-embed-text`.
#ifndef BundleModelFalse
Source: "staging\fat\models\nomic-embed-text.gguf"; DestDir: "{app}\models"; Flags: ignoreversion skipifsourcedoesntexist
#endif
; cosign-verify-mini.exe: statically-linked ~3 MB binary (B4). Extracted before payload
; so InitializeSetup can run it before extraction — this entry must be in BOTH lite and fat.
; Listed here in the fat section as well for completeness; the lite [Code] block references
; the same binary from the MSI's temp extraction path.
Source: "staging\fat\cosign-verify-mini.exe"; DestDir: "{tmp}"; DestName: "cosign-verify-mini.exe"; Flags: ignoreversion skipifsourcedoesntexist deleteafterinstall
#endif

[Run]
; Silent MSI install. The MSI itself owns the binary placement + tray autostart Run-key + bundled ollama+qdrant.
Filename: "msiexec.exe"; Parameters: "/i ""{tmp}\amore-windows-x64.msi"" /qn /norestart"; Flags: runascurrentuser waituntilterminated; StatusMsg: "Installing Amore..."
; Auto-wire AI IDEs after MSI places binaries. Fires under both interactive and /VERYSILENT installs.
; Non-zero exit from any entry fails the installer (no Flags: dontfailonprepare) so wiring errors are loud.
; NOTE: --self-contained flag requires Phase A6 (amore-mcp self-contained bundle); registered here as
; future-state. Until A6 lands, amore-mcp falls back gracefully if flag is unrecognised.
Filename: "{app}\amore-gui.exe"; Parameters: "--auto-wire"; Flags: runhidden runascurrentuser waituntilterminated; StatusMsg: "Detecting and wiring AI IDEs..."
Filename: "{app}\amore-mcp.exe"; Parameters: "--register-claude-code --self-contained"; Flags: runhidden runascurrentuser waituntilterminated
Filename: "{app}\amore-mcp.exe"; Parameters: "--register-claude-desktop --self-contained"; Flags: runhidden runascurrentuser waituntilterminated

[UninstallRun]
; Mirror uninstall via msiexec /x to keep MSI ownership.
Filename: "msiexec.exe"; Parameters: "/x ""{tmp}\amore-windows-x64.msi"" /qn /norestart"; Flags: runascurrentuser waituntilterminated; RunOnceId: "AmoreMsiUninstall"
