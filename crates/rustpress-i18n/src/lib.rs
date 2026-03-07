//! # rustpress-i18n
//!
//! Internationalization (i18n) system for RustPress, compatible with WordPress .mo files
//! and translation functions (`__()`, `_e()`, `_n()`, `_x()`, `_nx()`).
//!
//! This crate provides:
//! - GNU gettext `.mo` file parsing
//! - Plural form expression evaluation
//! - Locale detection and management
//! - WordPress-compatible translation functions

pub mod locale;
pub mod mo_parser;
pub mod plural;
pub mod translator;

pub use locale::{Locale, LocaleManager};
pub use mo_parser::{MoError, MoFile};
pub use plural::PluralExpression;
pub use translator::Translator;
