//! Tri-state field for JSON:API `PATCH` partial-update semantics.
//!
//! A JSON:API `PATCH` is a *partial* update: only members **present** in the
//! request body change, an explicit `null` clears a member, and absent members
//! are left untouched. A normal typed field cannot represent this — `serde`
//! collapses an absent key and an explicit `null` into the same `None`, and a
//! required field forces the member to be present at all. [`Field`] captures the
//! three states a PATCH member can be in:
//!
//! - [`Field::Absent`] — the key was **not** in the request → leave unchanged.
//! - [`Field::Null`] — the key was present with value `null` → clear the member.
//! - [`Field::Set`] — the key was present with a value → assign it.
//!
//! [`Field`] is meant to be used through `#[derive(JsonApi)]`: declare the
//! patchable members of a companion "patch" struct as `Field<T>`, and the derive
//! generates presence-aware (de)serialization for them. It has **no** standalone
//! `serde` impls — presence is a decision the surrounding resource envelope
//! makes (skip the key vs. emit `null`), which a value's own `Serialize` cannot
//! express. See the crate-level docs and `jsonapi_axum`'s PATCH example for the
//! end-to-end flow.
//!
//! ```
//! use jsonapi_core::Field;
//!
//! # #[derive(Clone)] struct Article { title: String, summary: Option<String> }
//! // Applying a patch onto a domain entity:
//! # let mut article = Article { title: "old".into(), summary: Some("s".into()) };
//! let title: Field<String> = Field::Set("new title".into());
//! let summary: Field<String> = Field::Null; // clear it
//!
//! if let Some(new_title) = title.into_set() {
//!     article.title = new_title; // a non-nullable column: only SET changes it
//! }
//! summary.apply(&mut article.summary); // ABSENT leaves, NULL → None, SET → Some
//! assert_eq!(article.summary, None);
//! ```

/// A JSON:API `PATCH` member in one of three states: [`Absent`](Field::Absent)
/// (leave unchanged), [`Null`](Field::Null) (clear), or [`Set`](Field::Set)
/// (assign a value).
///
/// Defaults to [`Absent`](Field::Absent). See the [module docs](self) for the
/// rationale and usage through `#[derive(JsonApi)]`.
///
/// # Relationships
///
/// `Field` may wrap a relationship (`Field<Relationship<T>>`): `Absent` leaves
/// the relationship untouched and `Set(rel)` **replaces** it. To clear a to-one
/// relationship, `Set` a relationship whose linkage is null (`{"data": null}`) —
/// **not** [`Field::Null`], which would emit a bare `"rel": null`, not a valid
/// JSON:API relationship object. To-many append/remove is a
/// relationship-endpoint concern, not a resource `PATCH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field<T> {
    /// The member was not present in the request; leave it unchanged.
    Absent,
    /// The member was present with value `null`; clear it.
    Null,
    /// The member was present with a value; assign it.
    Set(T),
}

impl<T> Default for Field<T> {
    /// A missing member defaults to [`Absent`](Field::Absent).
    fn default() -> Self {
        Field::Absent
    }
}

impl<T> Field<T> {
    /// Construct a [`Set`](Field::Set) field. Equivalent to `Field::Set(value)`,
    /// provided for symmetry with the query-style helpers.
    pub fn set(value: T) -> Self {
        Field::Set(value)
    }

    /// Returns `true` if the field is [`Absent`](Field::Absent).
    #[must_use]
    pub fn is_absent(&self) -> bool {
        matches!(self, Field::Absent)
    }

    /// Returns `true` if the field is [`Null`](Field::Null).
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Field::Null)
    }

    /// Returns `true` if the field is [`Set`](Field::Set).
    #[must_use]
    pub fn is_set(&self) -> bool {
        matches!(self, Field::Set(_))
    }

    /// Borrow the contained value when the field is [`Set`](Field::Set).
    #[must_use]
    pub fn as_set(&self) -> Option<&T> {
        match self {
            Field::Set(value) => Some(value),
            _ => None,
        }
    }

    /// Consume the field, yielding the contained value only when it is
    /// [`Set`](Field::Set). [`Absent`](Field::Absent) and [`Null`](Field::Null)
    /// both map to `None` — use this when a non-nullable target should change
    /// only on an explicit set.
    #[must_use]
    pub fn into_set(self) -> Option<T> {
        match self {
            Field::Set(value) => Some(value),
            _ => None,
        }
    }

    /// Map the contained value, preserving the [`Absent`](Field::Absent) /
    /// [`Null`](Field::Null) / [`Set`](Field::Set) state.
    #[must_use]
    pub fn map<U, F>(self, f: F) -> Field<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Field::Absent => Field::Absent,
            Field::Null => Field::Null,
            Field::Set(value) => Field::Set(f(value)),
        }
    }

    /// Return the contained value if [`Set`](Field::Set), otherwise `default`.
    #[must_use]
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Field::Set(value) => value,
            _ => default,
        }
    }

    /// Apply this patch member onto a nullable target `slot`:
    ///
    /// - [`Absent`](Field::Absent) leaves `slot` unchanged,
    /// - [`Null`](Field::Null) sets `slot` to `None`,
    /// - [`Set(v)`](Field::Set) sets `slot` to `Some(v)`.
    ///
    /// This is the canonical way to write a `Field<T>` onto an `Option<T>` column
    /// of a domain entity while honoring all three PATCH states.
    pub fn apply(self, slot: &mut Option<T>) {
        match self {
            Field::Absent => {}
            Field::Null => *slot = None,
            Field::Set(value) => *slot = Some(value),
        }
    }
}

impl<T> From<T> for Field<T> {
    /// A bare value becomes [`Set`](Field::Set). (There is intentionally no
    /// `From<Option<T>>`: mapping `None` to either `Absent` or `Null` would be a
    /// silent guess between two distinct PATCH intents.)
    fn from(value: T) -> Self {
        Field::Set(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_absent() {
        let field: Field<i32> = Field::default();
        assert!(field.is_absent());
    }

    #[test]
    fn predicates_match_variant() {
        assert!(Field::<i32>::Absent.is_absent());
        assert!(Field::<i32>::Null.is_null());
        assert!(Field::Set(1).is_set());

        assert!(!Field::<i32>::Absent.is_set());
        assert!(!Field::<i32>::Null.is_absent());
    }

    #[test]
    fn as_set_and_into_set_only_yield_for_set() {
        assert_eq!(Field::Set(7).as_set(), Some(&7));
        assert_eq!(Field::<i32>::Null.as_set(), None);
        assert_eq!(Field::<i32>::Absent.as_set(), None);

        assert_eq!(Field::Set(7).into_set(), Some(7));
        assert_eq!(Field::<i32>::Null.into_set(), None);
        assert_eq!(Field::<i32>::Absent.into_set(), None);
    }

    #[test]
    fn map_preserves_state() {
        assert_eq!(Field::Set(2).map(|v| v * 10), Field::Set(20));
        assert_eq!(Field::<i32>::Null.map(|v| v * 10), Field::Null);
        assert_eq!(Field::<i32>::Absent.map(|v| v * 10), Field::Absent);
    }

    #[test]
    fn unwrap_or_uses_default_for_absent_and_null() {
        assert_eq!(Field::Set(5).unwrap_or(0), 5);
        assert_eq!(Field::<i32>::Null.unwrap_or(0), 0);
        assert_eq!(Field::<i32>::Absent.unwrap_or(0), 0);
    }

    #[test]
    fn apply_honors_all_three_states() {
        // Absent leaves the slot untouched.
        let mut slot = Some(1);
        Field::<i32>::Absent.apply(&mut slot);
        assert_eq!(slot, Some(1));

        // Null clears it.
        let mut slot = Some(1);
        Field::<i32>::Null.apply(&mut slot);
        assert_eq!(slot, None);

        // Set assigns it.
        let mut slot = None;
        Field::Set(9).apply(&mut slot);
        assert_eq!(slot, Some(9));
    }

    #[test]
    fn from_value_is_set() {
        let field: Field<i32> = 3.into();
        assert_eq!(field, Field::Set(3));
    }
}
