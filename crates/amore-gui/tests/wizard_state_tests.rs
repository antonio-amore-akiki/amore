// crates/amore-gui/tests/wizard_state_tests.rs wizard state-machine tests.
//
// Tests the 6-screen Next/Back state transitions and can_advance() gating.
// No egui context needed — all logic is pure state mutation.

use amore_gui::wizard::{Screen, WizardState};

// ── Screen chain ──────────────────────────────────────────────────────────────

#[test]
fn screen_next_chain_complete() {
    let chain = [
        Screen::Welcome,
        Screen::DataDir,
        Screen::BundledDeps,
        Screen::IdeDetect,
        Screen::WireConfirm,
        Screen::Done,
    ];
    for window in chain.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        assert_eq!(a.next().as_ref(), Some(b), "{a:?}.next() should be {b:?}");
    }
    assert!(Screen::Done.next().is_none(), "Done.next() should be None");
}

#[test]
fn screen_prev_chain_complete() {
    let chain = [
        Screen::Welcome,
        Screen::DataDir,
        Screen::BundledDeps,
        Screen::IdeDetect,
        Screen::WireConfirm,
        Screen::Done,
    ];
    assert!(Screen::Welcome.prev().is_none(), "Welcome.prev() should be None");
    for window in chain.windows(2) {
        let (a, b) = (&window[0], &window[1]);
        assert_eq!(b.prev().as_ref(), Some(a), "{b:?}.prev() should be {a:?}");
    }
}

// ── can_advance() gating ──────────────────────────────────────────────────────

#[test]
fn welcome_blocked_until_license_accepted() {
    let mut state = WizardState::new();
    assert_eq!(state.screen, Screen::Welcome);
    assert!(!state.can_advance(), "should be blocked before license accepted");
    state.license_accepted = true;
    assert!(state.can_advance(), "should be unblocked after license accepted");
}

#[test]
fn data_dir_blocked_when_empty() {
    let mut state = WizardState::new();
    state.screen = Screen::DataDir;
    state.data_dir = String::new();
    assert!(!state.can_advance(), "should be blocked when data_dir is empty");
    state.data_dir = "/tmp/amore".to_string();
    assert!(state.can_advance(), "should be unblocked when data_dir is set");
}

#[test]
fn bundled_deps_and_ide_detect_always_advance() {
    let mut state = WizardState::new();
    state.screen = Screen::BundledDeps;
    assert!(state.can_advance(), "BundledDeps should always allow advance");
    state.screen = Screen::IdeDetect;
    assert!(state.can_advance(), "IdeDetect should always allow advance");
}

#[test]
fn wire_confirm_and_done_cannot_advance() {
    let mut state = WizardState::new();
    state.screen = Screen::WireConfirm;
    assert!(!state.can_advance(), "WireConfirm: Next replaced by Apply — must not advance via can_advance()");
    state.screen = Screen::Done;
    assert!(!state.can_advance(), "Done has no Next");
}

// ── Full 6-screen Next simulation ─────────────────────────────────────────────

#[test]
fn simulate_full_wizard_next_flow() {
    let mut screen = Screen::Welcome;

    // S1: accept license
    assert!(Screen::Welcome.prev().is_none());
    screen = screen.next().expect("Welcome -> DataDir");
    assert_eq!(screen, Screen::DataDir);

    // S2: data dir filled (always has default)
    screen = screen.next().expect("DataDir -> BundledDeps");
    assert_eq!(screen, Screen::BundledDeps);

    // S3: no gate
    screen = screen.next().expect("BundledDeps -> IdeDetect");
    assert_eq!(screen, Screen::IdeDetect);

    // S4: no gate
    screen = screen.next().expect("IdeDetect -> WireConfirm");
    assert_eq!(screen, Screen::WireConfirm);

    // S5: WireConfirm — next() exists but UI uses Apply button + explicit Done transition
    screen = screen.next().expect("WireConfirm -> Done");
    assert_eq!(screen, Screen::Done);

    // S6: no next
    assert!(screen.next().is_none());
}

#[test]
fn simulate_back_from_wire_confirm() {
    let mut screen = Screen::WireConfirm;
    screen = screen.prev().expect("WireConfirm -> IdeDetect");
    assert_eq!(screen, Screen::IdeDetect);
    screen = screen.prev().expect("IdeDetect -> BundledDeps");
    assert_eq!(screen, Screen::BundledDeps);
    screen = screen.prev().expect("BundledDeps -> DataDir");
    assert_eq!(screen, Screen::DataDir);
    screen = screen.prev().expect("DataDir -> Welcome");
    assert_eq!(screen, Screen::Welcome);
    assert!(screen.prev().is_none());
}
