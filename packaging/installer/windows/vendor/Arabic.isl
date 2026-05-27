; Arabic.isl — Unofficial Inno Setup Arabic translation stub.
;
; TODO (BEFORE NEXT WINDOWS BUILD):
;   1. Download the real Arabic.isl from the unofficial translations repository:
;      https://github.com/jrsoftware/unofficial-inno-setup-translations
;      (file: Arabic.isl — community-maintained, ISC-equivalent license)
;   2. Verify SHA256 of the downloaded file and pin it below.
;   3. Replace this stub with the real file content.
;
; Expected SHA256 (pin once obtained):
;   SHA256 = <pin-after-download>
;
; This stub satisfies the iscc build dependency so CI does not fail on a missing file.
; The installer will compile but will display English strings for Arabic-locale users
; until the real translation file is substituted.
;
; When the real file is vendored, record in commit message:
;   "chore(installer): vendor real Arabic.isl SHA256=<hash>"
;
; Minimal stub — iscc requires LanguageName at minimum:

[LangOptions]
LanguageName=Arabic
LanguageID=$0401
LanguageCodePage=1256

[Messages]
; All messages intentionally left to fall back to compiler defaults until the
; real Arabic translation file is vendored. iscc does not require every message
; key to be present in a supplemental .isl.
