<!-- stable: true -->
# Accessibility Statement — amore-gui

## Scope

`amore-gui` is the egui-based first-run wizard + system-tray application. CLI (`amore`) inherits terminal accessibility from the host shell. MCP server is headless.

## Standards applied

- **WCAG 2.2 Level AA** (w3.org/TR/WCAG22/) — aspirational mapping for desktop GUI; WCAG is by spec a web standard, applied here as design guidance.
- **Microsoft MSAA / UI Automation** (learn.microsoft.com/en-us/windows/win32/winauto/) — actual standard for Windows native accessibility.
- **AccessKit** (github.com/AccessKit/accesskit) — cross-platform accessibility tree exposed by egui; integrates with UIA on Windows, AT-SPI on Linux, NSAccessibility on macOS.

## WCAG 2.2 AA mapping (aspirational)

| Criterion | Status | Notes |
|---|---|---|
| 1.4.3 Contrast (text) ≥ 4.5:1 | PARTIAL | egui dark/light themes meet contrast; custom themes may not — audit pending |
| 1.4.11 Non-text contrast ≥ 3:1 | PARTIAL | egui default UI elements meet; custom widgets pending audit |
| 2.4.7 Focus indicator visible | PASS | egui default focus indicator |
| 2.4.11 Focused component not obscured | PASS | egui auto-scrolls to keep focused element visible |
| 4.1.2 Name/role/value | PARTIAL | depends on AccessKit integration version (egui must include AccessKit) |
| 2.5.8 Target size minimum | PASS | egui default button sizing ≥ 24×24 |

## Known gaps

- Keyboard navigation NOT exhaustively tested across all dialogs
- Screen reader audit NOT performed (Windows Narrator / NVDA / Linux Orca / macOS VoiceOver)
- Color-blind palette NOT explicitly verified (deuteranopia / protanopia / tritanopia)
- No dedicated high-contrast theme variant

## Roadmap

- v1.1 plan: dedicated accessibility audit + screen-reader smoke test on Windows + Linux Orca
- v1.2 plan: high-contrast theme variant + reduced-motion option

## How to report accessibility issues

File via GitHub Issues using the Bug template (.github/ISSUE_TEMPLATE/bug.md) with the `a11y` label.

## Source
- w3.org/TR/WCAG22/
- learn.microsoft.com/en-us/windows/win32/winauto/accessibility-best-practices
- github.com/AccessKit/accesskit
- github.com/emilk/egui (egui upstream accessibility integration)
