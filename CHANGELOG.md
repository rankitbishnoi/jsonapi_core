# Changelog

All notable changes to `jsonapi_core` and `jsonapi_core_derive` will be
documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
within the bounds described in the [versioning policy](./README.md#versioning-policy).

The two crates in this workspace (`jsonapi_core`, `jsonapi_core_derive`) are
versioned in lockstep via `workspace.package.version`. Versions in this file
refer to that shared workspace version.

## [Unreleased]

## [0.2.1] — 2026-04-25

Additive consumer-DX release. All changes are source-compatible with 0.2.0;
new API is additive and gated behind `#[non_exhaustive]` extension points.

### Added

- **`Document` accessors** removing the
  `match Document::Data { data: PrimaryData::Single(p), .. } => Ok(*p)`
  boilerplate at every consumer call site:
  - `Document::into_single` / `into_many` / `into_meta` — consuming.
  - `Document::as_single` / `as_many` / `primary` / `included` — borrowing.
  Errors surface as a new `Error::UnexpectedDocumentShape { expected, found }`
  variant so consumers can map shape mismatches to the right HTTP status
  (e.g. 422 vs 502).
- New `Error::UnexpectedDocumentShape` variant.
- **`Links` inherent helpers** so consumers no longer reach into the public
  `.0` field:
  - `Links::contains(rel)` — key presence (counts `null` entries).
  - `Links::get(rel) -> Option<&Link>` — flattens "absent" and "present-but-null"
    into `None`; use `links.0.get(rel)` if you need to distinguish them.
  - `Links::iter()` — `(name, &Link)` pairs, skipping `null` values.
  - `Links::keys()`, `Links::len()`, `Links::is_empty()`, `Links::new()`.
  - `Links` now also derives `Default`.
- **`HasLinks` / `HasMeta` accessor traits** auto-implemented by the derive
  macro when `#[jsonapi(links)]` / `#[jsonapi(meta)]` fields are present.
  Resources without those fields do not implement the traits — the absence
  is part of the type's contract (verified via a compile-fail test). The
  dynamic `Resource` fallback also implements both. Consumers no longer hand-roll
  per-resource accessor traits to bound generic code on resource-level
  metadata.
- **`Relationship::single_id`** — type-checked to-one access. Returns the
  server-assigned id, or one of `Error::NullRelationship`,
  `Error::LidNotIndexed`, or `Error::RelationshipCardinalityMismatch` when
  the relationship cannot deliver a single server id.
- **Typed parse errors** via new `Document::from_str` / `from_slice` /
  `from_value` constructors that run a structural pre-pass before delegating
  to the existing `Deserialize` impl. Replaces opaque `serde_json::Error`
  text with four new structured variants:
  - `Error::TypeMismatch { expected, got, location }` — the wire-side
    `data.type` (or any `data[i].type`) does not match the type declared on
    the Rust resource.
  - `Error::MalformedRelationship { name, location, reason }` — a
    relationship object is structurally invalid (non-object value, or `data`
    that is not null/object/array).
  - `Error::MissingAttribute { resource_type, attribute, location }` — a
    non-`Option`, non-`Vec` attribute on the consumer's struct is absent
    from the wire `attributes` block on the primary resource.
  - `Error::IncludedRefMissing { name, type_, id, location }` — a
    primary-resource relationship references a `(type, id)` pair not present
    in the wire `included` array. Intentionally primary-data-only —
    references inside an included resource are not validated, because
    partial-include APIs routinely return compound documents whose included
    resources reference other resources the consumer did not request.
    Skipped silently when `included` is absent or empty, and for `lid`-only
    references (atomic-ops resolves those at request execution time).
  Consumers can map these to specific HTTP statuses (e.g. 502 for upstream
  type drift, 422 for required-attribute mismatches) instead of
  one-size-fits-all parse errors. The pre-pass is a no-op for
  `Document<Resource>` on the type and required-attribute checks (open-set
  primary type); the relationship walk and `IncludedRefMissing` check still
  run.
- **`TypeInfo::required_attribute_names`** — new `&'static [&'static str]`
  field on `TypeInfo`, populated automatically by the derive (filter:
  attribute fields that are neither `Option` nor `Vec`). Backed by a new
  `TypeInfo::with_required_attributes(...)` builder method so manual
  `TypeInfo::new(...)` callers stay source-compatible.

## [0.2.0] — 2026-04-24

The first DX-focused release. Bundles five derive and model improvements
surfaced during a real-world Drupal integration.

### Added

- **`Document<P, I = Resource>`** — `Document` is now generic over the primary
  type `P` *and* the included type `I`. The default `I = Resource` keeps the
  `included` array open-set, which matches real-world compound documents
  (heterogeneous authors, comments, tags). Existing `Document<Resource>` and
  `Document<Article>` call sites are unaffected because of the default.
- **`Relationship<T>` helpers** — `identifiers()`, `ids()`, `first_id()`,
  `first_id_or_lid()`. Consumers no longer need to `match` on the
  `#[non_exhaustive]` `RelationshipData` variants for common cases.
- **`Identity` accessors** — `as_id()` and `as_lid()` returning `Option<&str>`
  so consumers don't pattern-match on `#[non_exhaustive]` `Identity`.
- Pinning regression tests in `m3_derive.rs` covering null-tolerant Option
  attributes, sharper field-naming errors, and the
  `Vec<Relationship<T>>` array-of-wrappers shape (~400 lines).

### Changed

- **Derive: `Option<T>` attribute fields now accept wire `null`.** Previously
  errored with `invalid type: null, expected a string`. Pass-through semantics
  for `Option<serde_json::Value>` and `Option<Option<T>>` are preserved.
- **Derive: deserialization errors now name the offending wire field.** Errors
  surface as `field "foo": <inner error>` instead of bare serde messages.
- **`Document::deserialize` locates type mismatches.** Errors are now prefixed
  `in primary data: ...` or `in included[N]: ...` so the failing position is
  explicit.

### Fixed

- `docs/relationships.md` — corrected to reflect that `Vec<Relationship<T>>`
  parses the non-standard array-of-wrappers shape, not JSON:API to-many.

### Notes

This release does not break source compatibility for the supported call sites
documented through 0.1.x (typed `Document<T>` and dynamic `Document<Resource>`).
The added type parameter on `Document` is defaulted, so existing `Document<T>`
declarations continue to compile without changes.

## [0.1.2] — 2026-04-16

### Fixed

- Republished `jsonapi_core` and `jsonapi_core_derive` with `README` files
  attached to each crate's crates.io landing page (`readme = "../README.md"` in
  `[package]`).
- Pinned `jsonapi_core_derive` dependency from `jsonapi_core` to an exact
  version to prevent crates.io publish skew.

## [0.1.1] — 2026-04-16

### Added

- CI and release GitHub workflows (`ci.yml`, `release.yml`) for automated
  testing and crate publication.
- Comprehensive documentation suite under `docs/` (mdBook layout):
  introduction, defining resources, documents, relationships, registry +
  resolver, query builder, sparse fieldsets, content negotiation, atomic
  operations, error handling, member name validation, and a cookbook.
- Crate-level rustdoc tutorial with doctests in `lib.rs`.
- Field-level and item-level rustdoc on every public type and method
  (`#![warn(missing_docs)]`).
- Five runnable examples under `jsonapi_core/examples/`: `basic_serialize`,
  `basic_deserialize`, `dynamic_resource`, `query_builder`,
  `content_negotiation`, `atomic_operations`.

### Changed

- Examples and tests now compile conditionally on the `derive` feature, so a
  `--no-default-features` build succeeds.
- **MSRV bumped from 1.85 to 1.88** (CI and `Cargo.toml` `rust-version`).
- Replaced internal `HashMap` usage with `BTreeMap` in serialization paths for
  deterministic output ordering.
- `pub use model::*` replaced with explicit re-exports at the crate root.
- Atomic operations: `validate_lid_refs` returns structured `Error` variants
  instead of strings.

### Added (atomic operations extension)

- `atomic-ops` feature flag and `atomic` module implementing the JSON:API 1.1
  Atomic Operations extension: `AtomicRequest`, `AtomicResponse`,
  `AtomicResult`, `AtomicOperation`, `OperationTarget`, `OperationRef`,
  `validate_lid_refs`, `ATOMIC_EXT_URI`.
- Integration tests covering spec parity and `lid` cross-references.

### Added (sparse fieldsets + include paths)

- `TypeRegistry` and `TypeInfo` for static type metadata.
- `validate_include_paths()` walks the relationship graph.
- `FieldsetConfig` builder.
- `SparseSerializer<T>` for typed sparse fieldset filtering.
- `sparse_filter()` for dynamic `Value`-based fieldset filtering.

### Added (resolver)

- `Registry::resolve()` — kitsu-core-style flattened output with cycle
  detection, configurable `max_depth` via `ResolveConfig`.
- `Registry::get_all()` — type-only lookup, deserialization-skip on shape
  mismatch.

### Added (Rust-standards quality sweep)

- `#[non_exhaustive]` on every public enum.
- `#[must_use]` on builders and lookup methods.
- `Hash` derive on `Identity` for use in `HashSet` / `HashMap`.
- `Debug` / `Clone` / `Default` derives across model types where appropriate.
- Compile-fail tests covering invalid derive usage.

### Fixed

- `derive`: duplicate-field errors now point at the offending field span.
- `derive`: rejects conflicting field annotations (e.g. `id` + `relationship`).
- `derive`: surfaces deserialization errors for present-but-malformed `Vec` fields.
- `derive`: rejects `#[jsonapi(type = "...")]` on non-relationship fields.
- `Resource::serialize` propagates serde errors instead of panicking.
- `Registry::from_included` skips entries without `id` instead of panicking.
- `media_type::to_header_value` escapes quotes for round-trip correctness.
- `m4` resolver: root resource added to the ancestor set so back-references
  are correctly detected as cycles.

### Performance

- `member_name` validation: replaced `Vec<char>` allocation with iterator pass.
- `resolve_identifier`: removed unnecessary `Value` clone.
- `ResourceIdentifier::serialize`: borrowing repr to avoid clones.

## [0.1.0] — 2026-04-15

Initial release.

### Added

- Full type model for JSON:API v1.1: `Document`, `Resource`,
  `ResourceIdentifier`, `Relationship`, `RelationshipData`, `Link`, `Links`,
  `LinkObject`, `Hreflang`, `ApiError`, `ErrorSource`, `ErrorLinks`,
  `JsonApiObject`, `Meta`, `Identity`.
- Custom `Serialize` / `Deserialize` impls covering the JSON:API envelope
  format (mutually exclusive `data` / `errors`, meta-only documents, null
  primary data, to-one / to-many relationships).
- `#[derive(JsonApi)]` proc-macro generating `ResourceObject`, `Serialize`,
  and `Deserialize` impls. Field-level attributes: `id`, `lid`, `relationship`,
  `meta`, `links`, `rename`, `skip`. Struct-level attributes: `type`, `case`.
- Fuzzy deserialization aliases: camelCase, snake_case, kebab-case,
  PascalCase variants of every field name accepted on input.
- Compile-time validation of `type` strings and `rename` values per JSON:API
  member-name rules.
- `Registry` for typed lookup of `included` resources with `get`, `get_many`,
  `get_by_id`.
- `QueryBuilder` for JSON:API-aware query strings with bracket encoding and
  RFC 3986 percent-encoding.
- Content negotiation: `validate_content_type`, `negotiate_accept`,
  `JsonApiMediaType`, including `ext` and `profile` parameter handling.
- Member name validation per JSON:API 1.1 rules: `validate_member_name`,
  `MemberNameKind` (with `AtMember` for namespaced members).
- `Error` enum with structured variants for registry, member-name, media-type,
  document-structure, and include-path failures.

[Unreleased]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rankitbishnoi/jsonapi_core/commit/b47dd17
