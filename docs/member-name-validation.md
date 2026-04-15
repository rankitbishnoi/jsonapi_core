# Member Name Validation

JSON:API 1.1 [places strict rules](https://jsonapi.org/format/#document-member-names)
on what characters may appear in member names — keys in `attributes`,
`relationships`, and meta. `jsonapi_core` validates member names in two places:

- **At compile time**, inside the derive macro, for the struct-level `type`
  string and any `#[jsonapi(rename = "...")]` value.
- **At runtime**, via the [`validate_member_name`] function, for keys you don't
  control statically (e.g. extracted from a request body or query string).

## The three kinds of name

`MemberNameKind` distinguishes the three legal shapes:

```rust
pub enum MemberNameKind {
    Standard,
    AtMember { namespace: String, member: String },
    ExtensionMember { namespace: String, member: String },
}
```

| Kind | Example | What it means |
|------|---------|---------------|
| `Standard` | `"first-name"`, `"title"` | Ordinary member name. |
| `AtMember` | `"@ext:comments"` | At-prefixed extension member. |
| `ExtensionMember` | `"atomic:operations"` | Extension-namespaced member declared via the `ext` parameter. |

## Validating a name

```rust
use jsonapi_core::{MemberNameKind, validate_member_name};

assert!(matches!(
    validate_member_name("first-name"),
    Ok(MemberNameKind::Standard),
));

match validate_member_name("@ext:comments")? {
    MemberNameKind::AtMember { namespace, member } => {
        assert_eq!(namespace, "ext");
        assert_eq!(member, "comments");
    }
    _ => unreachable!(),
}

match validate_member_name("atomic:operations")? {
    MemberNameKind::ExtensionMember { namespace, member } => {
        assert_eq!(namespace, "atomic");
        assert_eq!(member, "operations");
    }
    _ => unreachable!(),
}

// Empty / illegal names → Err
assert!(validate_member_name("").is_err());
assert!(validate_member_name("-leading-hyphen").is_err());
```

## Character rules in brief

- Globally-allowed characters: `[a-zA-Z0-9\u{0080}-\u{FFFF}]`.
- The first **and** last character of a standard name must be globally allowed.
- Interior characters may additionally be `-`, `_`, or space.
- Empty names are rejected.

`Standard` names follow these rules end-to-end. `AtMember` and `ExtensionMember`
apply them to the namespace and member parts independently.

## How the derive macro uses this

When you write:

```rust
#[derive(JsonApi)]
#[jsonapi(type = "12articles!")]   // ← compile error
struct Article { /* ... */ }
```

the macro fails compilation with `Error::InvalidMemberName` because the type
string violates the rules above. The same check applies to `rename` values:

```rust
#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(rename = "bad name!")]   // ← compile error
    title: String,
}
```

## When you'd validate at runtime

The runtime API matters when names come from outside your code:

- Parsing user-supplied filter or fields parameters that include a member name.
- Validating a custom server-defined extension's namespaced members.
- Building a generic schema layer that accepts arbitrary types at runtime.

For derived structs, the compile-time check has already ensured your types and
field renames are spec-legal — there's no need to call `validate_member_name`
yourself for those.
