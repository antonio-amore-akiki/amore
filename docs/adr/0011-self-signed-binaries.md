# 11. Self-sign Windows and macOS binaries until v1.0

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Windows SmartScreen and macOS Gatekeeper require binaries to be signed
by a trusted certificate authority in order to run without a warning
dialog. For non-technical users, a SmartScreen "Windows protected your
PC" dialog or a macOS "can't be opened because it is from an
unidentified developer" is a hard stop — they will not know to proceed.

Two code signing certificate types exist:

* **Windows EV Code Signing Certificate**: ~$300-500/year (DigiCert,
  Sectigo). Requires hardware token for EV. Immediately removes
  SmartScreen reputation warning.
* **Apple Developer ID**: $99/year. Required for macOS Gatekeeper
  notarisation (Apple signs a hash of the bundle; Gatekeeper checks the
  signature online).

At v0.3.1 pre-launch, neither certificate is in budget for the v0.4.0
milestone. The decision is whether to ship unsigned, self-signed, or
delay until paid certs are acquired.

## Decision Drivers

* v1.0.0 is the commercial launch milestone; paid certs are a v1.0
  budget item
* Self-sign + documented click-through is achievable before v1.0
* Unsigned binaries produce a worse user experience than self-signed
  (more alarming OS dialog wording)
* Non-technical users need a clear README section explaining the
  click-through
* CLAUDE.md: no fallback/workaround/degraded path — document the
  limitation honestly; do not pretend the warning is not there

## Considered Options

* Paid EV cert (Windows) + Apple Developer ID (macOS) now
* Self-sign with SmartScreen / Gatekeeper click-through documented
* Unsigned (no signing at all)

## Decision Outcome

Chosen option: **self-sign now; document the click-through path in
README and first-run UI; promote to paid certs at v1.0.0**.

Windows self-sign process:

```powershell
# ci/sign-windows.ps1
$cert = New-SelfSignedCertificate `
    -Subject "CN=Amore by Antonio, O=Obelion" `
    -Type CodeSigningCert `
    -CertStoreLocation Cert:\CurrentUser\My
signtool sign /fd SHA256 /a /v "dist\amore-windows-x86_64.exe"
```

macOS: the binary is ad-hoc signed with `codesign --sign -` (Apple
calls this an "ad-hoc signature"). Gatekeeper quarantine is cleared
via the README instruction: right-click → Open → Open Anyway.

The Amore first-run wizard surfaces a one-paragraph plain-English
explanation ("Windows blocked Amore because it's new software from an
independent developer…") and a "How to allow it" expandable section
with screenshots.

Paid certs are **blocked_on:user** — they require a paid account and
hardware token purchase that the user will initiate at v1.0 release.

### Consequences

* Good: installation works before v1.0 without certificate expenditure
* Good: first-run wizard removes the support burden for click-through
* Good: self-sign still provides integrity verification (binary has not
  been tampered with after signing, even if CA trust is absent)
* Bad: SmartScreen warning persists until enough users run the binary
  to build reputation (no EV cert = no reputation bypass)
* Bad: macOS right-click workaround is a multi-step friction point
* Bad: some enterprise MDM policies block binaries without EV certs;
  these deployments must whitelist Amore manually

## Pros and Cons of the Options

### Paid EV cert + Apple Developer ID now

* Good: SmartScreen warning fully removed (EV certs bypass reputation)
* Good: macOS Gatekeeper passes silently
* Bad: ~$400/year cost not in v0.4.0 budget
* Bad: EV cert requires hardware USB token (YubiKey or similar);
  CI signing pipeline becomes more complex
* Bad: Apple Developer ID enrollment takes 1-2 business days

### Self-signed + documented click-through (CHOSEN)

* Good: zero certificate cost before v1.0
* Good: integrity verification still present
* Good: README + first-run wizard mitigates non-technical user confusion
* Bad: SmartScreen warning remains; reputation builds slowly
* Bad: macOS Gatekeeper requires extra click-through steps
* Bad: enterprise MDM blockers require manual whitelist

### Unsigned

* Good: simplest — no signing toolchain
* Bad: OS dialog wording is worse ("Unknown publisher" vs the self-signed
  dialog which names "CN=Amore by Antonio")
* Bad: no integrity claim at all; supply-chain attack is undetectable

## More Information

* README section: `docs/INSTALL.md#windows-smartscreen` (v0.4.0)
* First-run wizard string: `crates/amore-gui/src/first_run.rs` (Phase G)
* Paid cert vendors: DigiCert (EV ~$499/yr), Sectigo (~$299/yr)
* Apple Developer enrollment: https://developer.apple.com/programs/
* v1.0 cert budget item tracked in `docs/ROADMAP.md`
* Self-sign CI script: `ci/sign-windows.ps1`, `ci/sign-macos.sh`
* This decision is accepted UNTIL v1.0.0; reopen at commercial launch
