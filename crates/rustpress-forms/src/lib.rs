//! # rustpress-forms
//!
//! Form builder and submission handler for RustPress.
//! A Contact Form 7 / Gravity Forms equivalent, providing:
//!
//! - A declarative form builder API
//! - HTML rendering with CSRF protection and validation attributes
//! - Server-side validation of form submissions
//! - In-memory submission storage and management

pub mod builder;
pub mod renderer;
pub mod submission;
pub mod validation;

pub use builder::{FieldConfig, FormBuilder, FormConfig, FormField};
pub use renderer::{render_field, render_form};
pub use submission::{FormSubmission, SubmissionStatus, SubmissionStore};
pub use validation::{validate_submission, ValidationError, ValidationRule};
