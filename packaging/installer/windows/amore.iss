; amore.iss — Inno Setup script that wraps amore-windows-x64.msi into a consumer-friendly .exe.
; Free toolchain (Inno Setup, ISC-equivalent license). User double-clicks the .exe; Inno's first-launch
; screen runs msiexec /i silently. Output: amore-windows-x64.exe at the same dir as ISCC was invoked.
;
; Build:  iscc /Q amore.iss      (after the MSI is in the cwd or via /DMsiPath=...)
; CI:     see .github/workflows/release.yml job windows-build (Inno via chocolatey).

#define AmoreVersion "1.0.0"

[Setup]
AppId={{6F2B3A1E-AAA0-4D5A-9BB1-AMORE10220250527}}
AppName=Amore
AppVersion={#AmoreVersion}
AppPublisher=Antonio
AppPublisherURL=https://github.com/antonio-amore-akiki/amore
AppSupportURL=https://github.com/antonio-amore-akiki/amore/issues
DefaultDirName={autopf}\Amore
DisableProgramGroupPage=yes
OutputBaseFilename=amore-windows-x64
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
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; The bundled MSI is extracted to {tmp} and removed after install completes.
; CI passes -DMsiPath=...; locally you can place amore-windows-x64.msi next to amore.iss.
#ifdef MsiPath
Source: "{#MsiPath}"; DestDir: "{tmp}"; DestName: "amore-windows-x64.msi"; Flags: deleteafterinstall
#else
Source: "amore-windows-x64.msi"; DestDir: "{tmp}"; Flags: deleteafterinstall
#endif

[Run]
; Silent MSI install. The MSI itself owns the binary placement + tray autostart Run-key + bundled ollama+qdrant.
Filename: "msiexec.exe"; Parameters: "/i ""{tmp}\amore-windows-x64.msi"" /qn /norestart"; Flags: runascurrentuser waituntilterminated; StatusMsg: "Installing Amore..."

[UninstallRun]
; Mirror uninstall via msiexec /x to keep MSI ownership.
Filename: "msiexec.exe"; Parameters: "/x ""{tmp}\amore-windows-x64.msi"" /qn /norestart"; Flags: runascurrentuser waituntilterminated; RunOnceId: "AmoreMsiUninstall"
