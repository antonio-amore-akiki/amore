# Icon Requirement — Linux AppImage

The AppImage build requires a PNG icon at:

    packaging/installer/linux/amore-icon.png

Minimum size: 256×256 px. Recommended: 512×512 px.

The `[package.metadata.appimage]` config in `crates/amore-gui/Cargo.toml` references
this path. Without it, `cargo appimage` will error with a missing icon complaint.

Place the final branded icon here before running `scripts/build-installer-linux.sh`.

The .deb and .rpm builds do not require an icon at build time (icon is referenced by
the .desktop entry at runtime via the icon theme lookup for `amore`).
