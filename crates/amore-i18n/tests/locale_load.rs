//! locale_load.rs -- integration tests for amore-i18n.
//!
//! Two test groups:
//!   1. each_locale_parses -- each .ftl file is syntactically valid (no parse errors).
//!   2. key_parity -- every key defined in en/main.ftl also exists in fr/nl/de/ar.

use amore_i18n::{I18n, Lang};

/// All supported locales.
const ALL_LANGS: &[Lang] = &[Lang::En, Lang::Fr, Lang::Nl, Lang::De, Lang::Ar];

/// Every key that must be present in every locale.
/// Mirrors the keys in locales/en/main.ftl.
const REQUIRED_KEYS: &[&str] = &[
    "app-name",
    "app-tagline",
    "app-version",
    "wizard-next",
    "wizard-back",
    "wizard-finish",
    "wizard-cancel",
    "wizard-welcome",
    "wizard-welcome-subtitle",
    "wizard-step-1",
    "wizard-step-2",
    "wizard-step-3",
    "wizard-step-4",
    "wizard-step-5",
    "install-success",
    "install-error",
    "install-progress",
    "install-components",
    "install-ollama-wait",
    "uninstall-confirm",
    "uninstall-success",
    "uninstall-in-progress",
    "observe-success",
    "observe-error",
    "recall-empty",
    "recall-results",
    "recall-query-placeholder",
    "tray-tooltip",
    "tray-open-dashboard",
    "tray-pause",
    "tray-resume",
    "tray-recent-activity",
    "tray-check-updates",
    "tray-quit",
    "status-healthy",
    "status-degraded",
    "status-offline",
    "doctor-ok",
    "doctor-fail",
    "error-network",
    "error-disk-full",
    "error-permission-denied",
    "error-unknown",
];

/// Each locale's .ftl must parse without panicking.
/// I18n::new panics if the embedded .ftl has parse errors.
#[test]
fn each_locale_parses() {
    for &lang in ALL_LANGS {
        let _i18n = I18n::new(lang);
        println!("ok: {:?} parsed cleanly", lang);
    }
}

/// Every key in REQUIRED_KEYS must be present in every locale's bundle.
/// Missing key causes test failure with actionable message.
#[test]
fn key_parity_across_all_locales() {
    let mut failures: Vec<String> = Vec::new();

    for &lang in ALL_LANGS {
        let i18n = I18n::new(lang);
        for &key in REQUIRED_KEYS {
            let result = i18n.try_t(
                key,
                &[
                    ("version", "1.0.0"),
                    ("reason", "test"),
                    ("percent", "50"),
                    ("count", "2"),
                    ("component", "test-component"),
                    ("path", "/test/path"),
                ],
            );
            if let Err(e) = result {
                failures.push(format!("  [{:?}] key '{}' => {:?}", lang, key, e));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Key parity failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    println!(
        "Key parity: {} keys x {} locales = {} checks passed",
        REQUIRED_KEYS.len(),
        ALL_LANGS.len(),
        REQUIRED_KEYS.len() * ALL_LANGS.len()
    );
}

/// t! macro -- spot-check on EN.
#[test]
fn t_macro_returns_string() {
    let i18n = I18n::new(Lang::En);
    let name = amore_i18n::t!(i18n, "app-name");
    assert_eq!(name, "Amore", "English app-name must be 'Amore'");
    let msg = amore_i18n::t!(i18n, "install-error", reason = "disk full");
    assert!(
        msg.contains("disk full"),
        "install-error must interpolate $reason"
    );
}

/// Missing key falls back to the key string -- never panics.
#[test]
fn missing_key_fallback() {
    let i18n = I18n::new(Lang::En);
    let result = i18n.t("key-that-does-not-exist", &[]);
    assert_eq!(result, "key-that-does-not-exist");
}

/// from_os_locale does not panic regardless of OS locale value.
#[test]
fn from_os_locale_does_not_panic() {
    let _i18n = I18n::from_os_locale();
}
