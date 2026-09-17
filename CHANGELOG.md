# Changelog

All notable changes to the workspace crates (`jsonapi_core`,
`jsonapi_core_derive`, `jsonapi_core_validation`, `jsonapi_http`, and
`jsonapi_axum`) are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
within the bounds described in the [versioning policy](./README.md#versioning-policy).

All five publishable crates in this workspace (`jsonapi_core`,
`jsonapi_core_derive`, `jsonapi_core_validation`, `jsonapi_http`, and
`jsonapi_axum`) are versioned in lockstep via `workspace.package.version` and
released together under one tag. Versions in this file refer to that shared
workspace version.

## [Unreleased]

### Added

- **New crate `jsonapi_axum`** — a first-class [axum](https://docs.rs/axum)
  adapter, now published. You can build a JSON:API service with typed extractors
  (`JsonApi<T>` request bodies, `JsonApiQuery` / `JsonApiQueryValidated`,
  `NegotiatedMediaType`, `BaseUrl`), responders (`JsonApiResponse` with
  `201`/`Location`/sparse-fieldset support, `RelationshipResponse`),
  relationship-endpoint extractors (`JsonApiToOne` / `JsonApiToMany`), and a
  complete CRUD surface including PATCH partial updates. Errors funnel through
  `JsonApiError` with `IntoJsonApiError` + `ResultExt::or_json_api` for clean `?`
  in handlers, with optional `validator` (`from_validation_errors`), `anyhow`,
  and `sqlx` conversions. Tower layers cover content negotiation (`JsonApiLayer`),
  error-body normalization (`NormalizeErrorsLayer`, `not_found`), and
  request-id / error-id correlation (`RequestIdLayer`). Self-links and the
  `Location` header build from `BaseUrl`; `pagination_links` assembles
  first/prev/next/last. In-process test helpers ship behind the `testing`
  feature. Feature flags: `validator`, `anyhow`, `sqlx`, `uuid`, `debug-errors`,
  `testing`.
- **New crate `jsonapi_http`** — the framework-agnostic HTTP layer the adapters
  build on, now published. Provides request parsing (`check_content_type`,
  `deserialize_body`, `negotiate`, `parse_query`), response building
  (`json_api_response` / `document_response` and the fallible `try_*` variants),
  error-document mapping (`status_for`, `to_api_error`, `error_response*`, and the
  `with_status` / `ApiErrorExt` / `ApiErrors` builders), content-negotiation tower
  layers (`ContentTypeLayer`, `AcceptLayer`, `JsonApiLayer`), client-id policy
  (`ClientIdPolicy`, `check_client_id`, `check_id_matches`, `id_conflict`),
  compound-document include resolution (`IncludeResolver`, `resolve_includes`),
  and request-id error-id stamping. Depend on it directly to write an adapter for
  another framework.
- **New crate `jsonapi_core_validation`** — shared member-name validation used by
  both `jsonapi_core` (runtime) and `jsonapi_core_derive` (compile time) so the
  two can no longer drift. An implementation detail published as a dependency;
  not intended for direct use.
- You can now build JSON:API responses fallibly: `jsonapi_http::try_json_api_response`,
  `try_json_api_response_filtered`, and `try_document_response` return a
  `Result` instead of panicking when a payload cannot be serialized to JSON
  (e.g. an attribute with non-string map keys). The infallible builders remain,
  now with documented `# Panics` sections pointing at these variants.

- Dynamic `Resource` round-trips are now lossless for relationships:
  relationship-level `links` and `meta` are preserved (previously dropped), and
  relationships carrying only `links`/`meta` with no `data` are retained. A new
  public `ResourceRelationship` type (`data: Option<RelationshipData>`, `links`,
  `meta`, `ResourceRelationship::new(data)`, and its own `Serialize`/`Deserialize`)
  models the untyped relationship object.
- Server-side query parsing: `Query` (`from_pairs` / `from_query_string`) parses
  `sort`, `include`, `fields[type]`, `page[...]`, and `filter[...]`, with
  `SortField` for sort direction. Filter and page stay generic (server-defined
  semantics); repeated `filter[...]` keys accumulate, repeated `page[...]` keys are
  last-wins; unknown parameters are ignored.
- Cursor-pagination profile: `CURSOR_PAGINATION_PROFILE`, `CursorPage` (a typed
  view over the `page` map), and `CursorLinks` (a first/prev/next/last pagination
  link builder that preserves request parameters).
- `DocumentBuilder` — a fluent builder for response documents (`data`/`many`/
  `no_data`, `include`/`include_many` with `(type, id)` de-duplication and
  primary-skip, `link`/`links`/`meta`/`jsonapi`/`profile`/`ext`), plus
  `Document::errors` and `Document::meta_only` constructors.
- New `Error::QueryParse` variant for malformed query parameters.
- `negotiate_accept` now honours HTTP `Accept` quality weights (`q`): the
  highest-weighted acceptable media type wins (ties: an explicit
  `application/vnd.api+json` beats a wildcard, then document order), and `q=0`
  marks a media type as not acceptable.
- `Resource` and `ResourceRelationship` now implement `Default`, so you can
  build them with struct-update syntax (`Resource { r#type: "articles".into(),
  ..Default::default() }`). `ResourceRelationship` is `#[non_exhaustive]`, so
  `Default` is the only way for downstream crates to construct its empty state.
- `Links::insert`, `Links::get_raw` (distinguishes absent / null / present
  entries), and `From<BTreeMap<String, Option<Link>>>` for populating a `Links`
  without touching the inner map.
- `Document::parse` now surfaces a document carrying both `data` and `errors`
  as the typed `Error::Structure` variant instead of an opaque `Error::Json`.
- More common trait impls: `Default` for `Identity`, `ResourceIdentifier`,
  `OperationRef`, and `SortField`; `Clone` for `Registry`; `Eq` for
  `FieldsetConfig`.
- `impl Display for JsonApiMediaType` (the canonical string form;
  `to_header_value` delegates to it).
- `impl From<Links>` and `From<Meta>` for `ResourceRelationship`.
- `Cardinality` enum (`ToOne` / `ToMany`), exported at the crate root.
- **Ergonomic linkage constructors** — build identifiers and relationships without
  struct literals: `ResourceIdentifier::new(type, id)` / `::with_lid(type, lid)` /
  `::many(type, ids)`, and `Relationship::to_one(rid)` / `to_one_id(type, id)` /
  `to_one_null()` / `to_many(rids)`. Read a to-one's id with
  `ResourceRelationship::single_id` and a required identifier with
  `ResourceIdentifier::require_id`.
- **Relationship target inference in the derive** — a new `ResourceType` trait
  (auto-implemented by `#[derive(JsonApi)]`, exposing `TYPE`) lets a bare
  `#[jsonapi(relationship)] author: Relationship<AuthorResource>` infer its JSON:API
  type from the target, so you no longer repeat `#[jsonapi(relationship, type = "…")]`.
- `jsonapi_axum::pagination_links_with_base` — a pagination link builder that uses a
  configured base URL, so the `self` link and `first`/`prev`/`next`/`last` all agree
  on host (e.g. behind a reverse proxy) instead of reconstructing it from the
  incoming request URI.
- **Atomic Operations responder for axum**: `AtomicJsonApiResponse`, behind a new
  `atomic-ops` feature on `jsonapi_axum`, serializes an `atomic:results` document.
- `JsonApiError::unprocessable(detail)` and
  `JsonApiError::unprocessable_with_pointer(pointer, detail)` shorthands for the
  common `422` cases (the latter also sets `source.pointer`).
- `Error::LidNotAllowed { r#type, lid }` — a client-facing error that names the
  offending reference when a relationship supplies a client `lid` where a
  server-assigned `id` is required (maps to `400`).
- **New end-to-end example — a JSON:API showcase** (`examples/`). Run a complete
  axum + SQLite JSON:API server together with a Leptos (WASM) single-page app that
  drives every endpoint over real cross-origin HTTP, with a live, self-captured
  inspector showing the exact `application/vnd.api+json` request/response for each
  action — all three pagination strategies, compound documents, sparse fieldsets,
  sort/filter, `Field<T>` PATCH, relationship endpoints, the error model, and atomic
  operations (with `lid` references). See `examples/README.md` to run it locally.

### Changed

- `jsonapi_http` and `jsonapi_axum` now share the workspace version and are
  released in lockstep with the other crates (both realign from `0.2.0` to the
  current workspace version).
- The `jsonapi_axum` responder (`JsonApiResponse`) now returns a JSON:API `500`
  error document instead of panicking when a response payload cannot be
  serialized to JSON. The raw serializer message is only included when the
  `debug-errors` feature is enabled.
- Member-name validation is now shared between `jsonapi_core` and
  `jsonapi_core_derive` through a new internal `jsonapi_core_validation` crate,
  so compile-time (derive) and runtime validation can no longer drift. This is
  an internal restructure with no behavior change for consumers.
- **Breaking:** `ResourceObject::type_info()` is now a required trait method. It
  previously had a default body that panicked at runtime; a missing
  implementation is now a compile error. Code using `#[derive(JsonApi)]` is
  unaffected — the macro generates it. Manual `ResourceObject` implementations
  must add `type_info()`.
- **Breaking:** `Resource.relationships` is now
  `BTreeMap<String, ResourceRelationship>` instead of
  `BTreeMap<String, RelationshipData>`. Access linkage via the `.data` field
  (e.g. `resource.relationships["author"].data`). An empty `{}` relationship
  object is now rejected on deserialize (JSON:API requires at least one of
  `data`, `links`, or `meta`).
- **Breaking:** `negotiate_accept` now returns the *chosen* `Accept` entry's
  requested `ext`/`profile` intersected with the server's capabilities, rather
  than the server's full capabilities on any match. A bare
  `application/vnd.api+json` request or a wildcard yields a plain response
  (empty `ext`/`profile`). Servers that relied on the old behaviour to advertise
  extensions must request them explicitly.
- **Breaking:** `Document::from_str` is renamed to `Document::parse`. The old
  name shadowed `std::str::FromStr::from_str`; `parse` is unambiguous. Update
  `Document::<T>::from_str(s)` call sites to `Document::<T>::parse(s)`.
- **Breaking:** `Document::as_single` / `as_many` are renamed to `try_as_single`
  / `try_as_many`. They return `Result`, so the `as_` prefix violated the Rust
  API guideline that `as_` accessors are infallible. The consuming
  `into_single` / `into_many` accessors are unchanged.
- **Breaking:** the inner `BTreeMap` of `Links` is now private. Use
  `Links::insert`, `Links::get_raw`, the other inherent accessors, or
  `Links::from(map)` instead of `links.0`.
- **Breaking:** `JsonApiMediaType` is now `#[non_exhaustive]`. Construct it via
  `validate_content_type` / `negotiate_accept` rather than a struct literal.
- **Breaking:** `JsonApiObject` and `ResolveConfig` are now `#[non_exhaustive]`.
  Build them via `Default` / the provided setters rather than a struct literal,
  and add a wildcard arm when exhaustively matching.
- **Breaking:** `Error::RelationshipCardinalityMismatch`'s `expected` field is
  now a typed `Cardinality` enum instead of `&'static str`. Match on
  `Cardinality::ToOne` / `Cardinality::ToMany` instead of the string literals.
- Client-origin error variants now map to `4xx` instead of `500`:
  `Error::NullRelationship` → `422` and `Error::LidNotIndexed` → `400`, so malformed
  client input surfaces as a client error rather than a server error.

### Deprecated

- `CursorLinks::build(...)` is deprecated in favour of the fluent
  `CursorLinks::first()/.prev()/.next()/.last()` setters + `.links()`, which read
  more clearly at the call site and are harder to pass positional booleans to wrong.

### Performance

- Fewer allocations on the serde hot paths: the `Document` deserializer now
  consumes the attributes map instead of cloning each field, and the parse
  pre-pass, registry resolution, and media-type parsing were reworked to avoid
  intermediate allocations. No behavioral change.

## [0.3.0] — 2026-09-13

### Added

- `Resource::from_typed(T)` now facilitates deriving a `Resource` from a typed `ResourceObject`.

### Changed

- **Breaking:** the JSON:API `type` member is now exposed as the raw-identifier
  field `r#type` instead of `type_` on `Resource`, `ResourceIdentifier`,
  `LinkObject`, `ErrorLinks`, and `OperationRef`, as well as on the
  `Error::RegistryLookup` and `Error::IncludedRefMissing` variants. Update field
  access and struct-literal construction from `type_` to `r#type`. The JSON wire
  format is unchanged.

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

### Packaging

- Published tarball trimmed: `jsonapi_core` now excludes `tests/**`
  (76 → 37 files). Test sources remain in the source repo for local and CI
  builds; only the registry artefact is leaner.
- `docs.rs` is configured to build both crates with `--all-features` so
  `atomic-ops` items are visible on the rendered documentation.
- `documentation` URL set on both crates' `[package]` metadata.

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
- Six runnable examples under `jsonapi_core/examples/`: `basic_serialize`,
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

[Unreleased]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/rankitbishnoi/jsonapi_core/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rankitbishnoi/jsonapi_core/commit/b47dd17
