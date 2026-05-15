<!-- stable: true -->
# Asset provenance — `docs/assets/`

Generated: 2026-05-27  
Method: synthetic mockups via PowerShell `System.Drawing` (Option B)  
Dimensions: 1280×800 px (wizard screens), 400×300 px (tray menu), 1280×400 px (hero composite)

Live captures from a running egui instance are deferred to v-next (see gap note below).

## File inventory

| File | Dimensions | Size | Depicts |
|---|---|---|---|
| `wizard-screen-1.png` | 1280×800 | 21 KB | Screen 1 — Welcome + Apache 2.0 license scroll + accept checkbox (unchecked) |
| `wizard-screen-2.png` | 1280×800 | 12 KB | Screen 2 — Data directory picker with free-space indicator |
| `wizard-screen-3.png` | 1280×800 | 16 KB | Screen 3 — Bundled components table (Ollama, Qdrant, disk estimates) |
| `wizard-screen-4.png` | 1280×800 | 19 KB | Screen 4 — IDE auto-detect grid with 4 detected tools (Claude Desktop, Claude Code, Cursor, Cline) |
| `wizard-ide-detect.png` | 1280×800 | 19 KB | Alias of `wizard-screen-4.png` — referenced by `docs/_readme-top-fragment.md` |
| `wizard-screen-5.png` | 1280×800 | 20 KB | Screen 5 — Review changes / wire confirmation with Apply button |
| `wizard-screen-6.png` | 1280×800 | 14 KB | Screen 6 — Done screen with green checkmark + Open dashboard + tray buttons |
| `tray-menu.png` | 400×300 | 4 KB | System tray context menu: Open dashboard / Pause / Resume / Recent activity / Check for updates / Quit |
| `amore-hero.png` | 1280×400 | 32 KB | Hero composite: Screen 4 (IDE detect) left panel + tray menu right panel, with caption labels |

## Generation method

Mockups produced with `System.Drawing.Graphics` in PowerShell 7+ on Windows 11.  
Color palette mirrors egui's default dark theme: background `#202020`, panel `#2d2d2d`, accent `#8a2be2` (blueviolet).  
Fonts: Segoe UI (body/headings), Consolas (code blocks).  
No hardcoded production data — all IDE paths and labels are illustrative placeholders matching the real screen layout from `crates/amore-gui/src/wizard/screens.rs`.

## Gap — live captures deferred to v-next

egui headless rendering on Windows requires a `wgpu` software device (no display server)
or `egui_backend` integration that is not yet in the project's dependency set.
Bootstrapping that path would add roughly a day of scope — deferred to v-next.

v-next action: add `wgpu` with `dx12`+`vulkan` features, write
`crates/amore-gui/src/bin/capture-screenshots.rs` using `egui::Context::run` +
`wgpu::Texture::copy_to_buffer`, replace these mockups with authentic captures.

## Usage in README

`docs/_readme-top-fragment.md` embeds two assets by name:
- `docs/assets/wizard-ide-detect.png` (Demo section, first screenshot)
- `docs/assets/tray-menu.png` (Demo section, second screenshot)

The  assembly step will stitch these into the final `README.md`.
The animated GIF placeholder (`docs/assets/amore-demo.gif`) is still absent;
the  assembler should replace the GIF `<img>` tag with a static screenshot
or leave it as an empty placeholder pending v-next live capture.
