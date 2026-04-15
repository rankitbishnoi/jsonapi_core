//! Member-name validation per JSON:API 1.1.
//!
//! [`validate_member_name()`] checks that a string conforms to the JSON:API 1.1
//! member-name rules: allowed characters (including Unicode U+0080+), `@`-member
//! syntax (`@namespace:member`), and start/end character restrictions.

mod member_name;

pub use member_name::{MemberNameKind, validate_member_name};
