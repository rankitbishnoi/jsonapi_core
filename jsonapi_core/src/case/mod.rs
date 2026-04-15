//! Output case convention configuration.
//!
//! [`CaseConvention`] defines the target casing for attribute and relationship
//! member names during serialization (camelCase, snake_case, kebab-case,
//! PascalCase, or pass-through). [`CaseConfig`] bundles the convention and is
//! used with `#[jsonapi(case = "...")]` on derive structs.

mod config;

pub use config::{CaseConfig, CaseConvention};
