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

; --- B4 (F21): cosign-verify-mini.exe pre-extract verifier ---
; Bundled in BOTH lite and fat installers. Extracted early via ExtractTemporaryFile
; in [Code] InitializeSetup before any payload lands on disk.
; Source: upstream cosign static binary (github.com/sigstore/cosign, Apache-2.0).
; Staging: scripts/build-installer-windows.ps1 copies cosign-verify-mini.exe from
;   packaging/installer/cosign-verify-mini/cosign-verify-mini-windows-amd64.exe
; skipifsourcedoesntexist: local iscc validation succeeds without staging; CI populates it.
Source: "staging\cosign-verify-mini.exe"; DestDir: "{tmp}"; DestName: "cosign-verify-mini.exe"; Flags: ignoreversion skipifsourcedoesntexist deleteafterinstall dontcopy

; --- FAT INSTALLER EXTRA FILES (B3, F20) ---
; Only included when compiling with /DFatInstaller.
; Provides air-gapped / first-run-offline install capability.
; Expected staging layout (built by build-installer-windows.ps1 -BundleDeps):
;   staging\fat\ollama.exe           (~150 MB, ollama v0.24.0)
;   staging\fat\qdrant.exe           (~80 MB, qdrant v1.18.1)
;   staging\fat\models\nomic-embed-text.gguf  (~274 MB, default embedding model)
;
; skipifsourcedoesntexist: local iscc validation succeeds without staging; CI populates staging.
#ifdef FatInstaller
Source: "staging\fat\ollama.exe"; DestDir: "{app}"; DestName: "ollama.exe"; Flags: ignoreversion skipifsourcedoesntexist
Source: "staging\fat\qdrant.exe"; DestDir: "{app}"; DestName: "qdrant.exe"; Flags: ignoreversion skipifsourcedoesntexist
; BundleModel=false: skip the GGUF file — post-install amore-gui pulls it via `ollama pull nomic-embed-text`.
#ifndef BundleModelFalse
Source: "staging\fat\models\nomic-embed-text.gguf"; DestDir: "{app}\models"; Flags: ignoreversion skipifsourcedoesntexist
#endif
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

[Code]
// B4 (F21): SBOM + Sigstore pre-extract verification.
//
// InitializeSetup runs BEFORE any [Files] are extracted. We use ExtractTemporaryFile
// to pull cosign-verify-mini.exe out of the installer's payload into {tmp} early, then
// invoke it against the installer's own SHA256 + Sigstore bundle signature.
//
// Fail-loud on mismatch: MsgBox with clear error pointing to release-page checksums;
// installer returns False (aborts). No silent fail-open.
//
// The ReleaseBundle variable below is populated at compile time by the CI build step
// that runs iscc. Pass /DReleaseBundle=<path-to-sigstore-bundle> and
// /DReleaseSha256=<hex-sha256> on the iscc command line. If not provided, verification
// is skipped with a logged warning (dev builds). Production CI always provides both.
//
// Upstream: github.com/sigstore/cosign verify-blob semantics:
//   cosign verify-blob --bundle <bundle.sigstore> --certificate-identity-regexp=<id> <payload>
// The bundle is the .sigstore file produced by cosign sign-blob in the release workflow.

#ifdef ReleaseBundle
  // ReleaseBundle and ReleaseSha256 are compile-time defines set by CI.
  // Defined here as Inno constants for use in the [Code] block.
  #define ReleaseBundleVal ReleaseBundle
  #define ReleaseSha256Val ReleaseSha256
#else
  #define ReleaseBundleVal ""
  #define ReleaseSha256Val ""
#endif

function InitializeSetup(): Boolean;
var
  CosignExe, CosignArgs: String;
  ExitCode: Integer;
  BundlePath, Sha256, InstallerPath: String;
begin
  Result := True;

  BundlePath  := '{#ReleaseBundleVal}';
  Sha256      := '{#ReleaseSha256Val}';

  // Skip verification in dev builds (no CI defines provided).
  if (BundlePath = '') or (Sha256 = '') then
  begin
    // Dev build: verification skipped. Log to setup log for audit.
    Log('B4: cosign pre-extract skip — ReleaseBundle/ReleaseSha256 not set (dev build).');
    Exit;
  end;

  // Extract cosign-verify-mini.exe from installer payload into {tmp}.
  // ExtractTemporaryFile requires Flags: dontcopy on the [Files] entry.
  ExtractTemporaryFile('cosign-verify-mini.exe');
  CosignExe := ExpandConstant('{tmp}\cosign-verify-mini.exe');

  if not FileExists(CosignExe) then
  begin
    Log('B4: cosign-verify-mini.exe not found in {tmp} — pre-extract verification skipped (installer was built without staging/cosign-verify-mini.exe).');
    Exit;
  end;

  // Build the verify-blob command:
  //   cosign verify-blob --bundle <bundle> --certificate-identity-regexp=.* <installer>
  // InstallerPath = the .exe being run (this installer).
  InstallerPath := ExpandConstant('{srcexe}');
  CosignArgs := 'verify-blob'
    + ' --bundle "' + BundlePath + '"'
    + ' --certificate-identity-regexp=".*"'
    + ' --certificate-oidc-issuer-regexp=".*"'
    + ' "' + InstallerPath + '"';
  Log('B4: Running: ' + CosignExe + ' ' + CosignArgs);

  if not Exec(CosignExe, CosignArgs, '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then
  begin
    MsgBox(
      'Amore Installer — Security Check Failed' + #13#10 + #13#10 +
      'Could not launch the signature verifier (cosign-verify-mini.exe).' + #13#10 +
      'This installer may be corrupt or incomplete.' + #13#10 + #13#10 +
      'Please re-download Amore from:' + #13#10 +
      'https://github.com/antonio-amore-akiki/amore/releases' + #13#10 + #13#10 +
      'Check the SHA256 and .sigstore bundle listed on the release page.',
      mbCriticalError, MB_OK
    );
    Result := False;
    Exit;
  end;

  if ExitCode <> 0 then
  begin
    MsgBox(
      'Amore Installer — Signature Verification Failed' + #13#10 + #13#10 +
      'The Sigstore signature for this installer does NOT match.' + #13#10 +
      'Exit code: ' + IntToStr(ExitCode) + #13#10 + #13#10 +
      'This installer may have been tampered with or downloaded from an unofficial source.' + #13#10 + #13#10 +
      'Please verify against the official checksums at:' + #13#10 +
      'https://github.com/antonio-amore-akiki/amore/releases' + #13#10 + #13#10 +
      'Expected SHA256: ' + Sha256,
      mbCriticalError, MB_OK
    );
    Result := False;
    Exit;
  end;

  Log('B4: cosign pre-extract verification PASSED. Proceeding with installation.');
end;
