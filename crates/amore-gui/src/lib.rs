// crates/amore-gui/src/lib.rs — thin re-export shim.
//
// Exposes the amore-gui modules as a library target so integration tests
// in tests/ can import them as `amore_gui::ide_detect`, etc.
// All logic lives in the named modules; this file is pure re-exports.

pub mod ide_detect;
pub mod ide_wire;
pub mod tray;
pub mod wizard;
