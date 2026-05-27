//! amore-i18n — runtime internationalization for Amore.
//!
//! Loads Project Fluent `.ftl` files bundled at compile time for five locales:
//! `en`, `fr`, `nl`, `de`, `ar`.
//!
//! # Usage
//!
//! ```rust
//! use amore_i18n::{I18n, Lang};
//!
//! let i18n = I18n::new(Lang::En);
//! let msg = i18n.t("app-name", &[]);
//! assert_eq!(msg, "Amore");
//! ```
//!
//! # Arabic / RTL note
//!
//! Arabic strings are encoded with Unicode FSI (U+2068) / PDI (U+2069) isolation
//! marks per the Fluent BiDi convention. The Inno installer and CLI terminal both
//! render full RTL via their host environment. The egui GUI renders Arabic text
//! correctly but its button layout remains LTR until egui ships native RTL support
//! (tracked at <https://github.com/emilk/egui/issues/1016>).

use fluent::{FluentArgs, FluentBundle, FluentResource};
use thiserror::Error;
use unic_langid::LanguageIdentifier;

/// A supported locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
    Nl,
    De,
    Ar,
}

impl Lang {
    /// Parse from a BCP 47 locale string (e.g. `"fr"`, `"ar-LB"`).
    /// Falls back to `Lang::En` for any unrecognised tag.
    pub fn from_locale(s: &str) -> Self {
        let tag = s.split(['_', '-']).next().unwrap_or("en").to_lowercase();
        match tag.as_str() {
            "fr" => Self::Fr,
            "nl" => Self::Nl,
            "de" => Self::De,
            "ar" => Self::Ar,
            _ => Self::En,
        }
    }

    fn bcp47(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
            Self::Nl => "nl",
            Self::De => "de",
            Self::Ar => "ar",
        }
    }

    fn ftl_source(self) -> &'static str {
        match self {
            Self::En => include_str!("../locales/en/main.ftl"),
            Self::Fr => include_str!("../locales/fr/main.ftl"),
            Self::Nl => include_str!("../locales/nl/main.ftl"),
            Self::De => include_str!("../locales/de/main.ftl"),
            Self::Ar => include_str!("../locales/ar/main.ftl"),
        }
    }
}

/// Error type for i18n operations.
#[derive(Debug, Error)]
pub enum I18nError {
    #[error("failed to parse language identifier '{0}'")]
    LangId(String),
    #[error("failed to parse .ftl resource for locale '{0}'")]
    FtlParse(String),
    #[error("message key '{0}' not found in locale '{1}'")]
    KeyNotFound(String, String),
}

/// Runtime i18n handle.
///
/// Backed by a [`FluentBundle`] loaded from `.ftl` files embedded at compile
/// time via `include_str!`. Construction is infallible for the five supported
/// locales; errors are returned only from [`I18n::try_t`] when a key is absent.
pub struct I18n {
    lang: Lang,
    bundle: FluentBundle<FluentResource>,
}

impl I18n {
    /// Create an `I18n` instance for the given locale.
    ///
    /// # Panics
    ///
    /// Panics only when the embedded `.ftl` source is syntactically invalid —
    /// this is caught by `cargo test -p amore-i18n` (locale_load test) before
    /// any release.
    pub fn new(lang: Lang) -> Self {
        let lang_id: LanguageIdentifier = lang
            .bcp47()
            .parse()
            .expect("hard-coded BCP47 tag is always valid");

        let mut bundle = FluentBundle::new(vec![lang_id]);

        let source = lang.ftl_source();
        let resource =
            FluentResource::try_new(source.to_owned()).unwrap_or_else(|(_res, errors)| {
                panic!(
                    "embedded .ftl for locale '{}' has parse errors: {:?}",
                    lang.bcp47(),
                    errors
                )
            });

        bundle
            .add_resource(resource)
            .expect("no duplicate message IDs in embedded .ftl");

        Self { lang, bundle }
    }

    /// Detect the OS locale and return a matching `I18n` instance.
    /// Falls back to English when the OS locale is unavailable or unmapped.
    pub fn from_os_locale() -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_owned());
        Self::new(Lang::from_locale(&locale))
    }

    /// Translate `key`, substituting `args` (name/value pairs).
    /// Returns the English key itself when the key is absent (never panics).
    pub fn t(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.try_t(key, args).unwrap_or_else(|_| key.to_owned())
    }

    /// Translate `key`, returning an error when the key is absent.
    pub fn try_t(&self, key: &str, args: &[(&str, &str)]) -> Result<String, I18nError> {
        let msg = self
            .bundle
            .get_message(key)
            .ok_or_else(|| I18nError::KeyNotFound(key.to_owned(), self.lang.bcp47().to_owned()))?;
        let pattern = msg
            .value()
            .ok_or_else(|| I18nError::KeyNotFound(key.to_owned(), self.lang.bcp47().to_owned()))?;

        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, *v);
        }

        let mut errors = vec![];
        let value = self
            .bundle
            .format_pattern(pattern, Some(&fluent_args), &mut errors);
        Ok(value.into_owned())
    }

    /// Returns the active [`Lang`].
    pub fn lang(&self) -> Lang {
        self.lang
    }
}

/// Convenience macro: `t!(i18n, "key")` or `t!(i18n, "key", arg1 = "val1")`.
///
/// Returns `String`. Never panics — missing key yields the key itself.
#[macro_export]
macro_rules! t {
    ($i18n:expr, $key:expr) => {
        $i18n.t($key, &[])
    };
    ($i18n:expr, $key:expr, $($name:ident = $val:expr),+ $(,)?) => {
        $i18n.t($key, &[$(( stringify!($name), $val )),+])
    };
}
