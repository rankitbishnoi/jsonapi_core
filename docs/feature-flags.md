# Feature Flags

`jsonapi_core` exposes two Cargo features.

## `derive` (default)

Re-exports the `#[derive(JsonApi)]` proc macro from the companion crate
`jsonapi_core_derive`. Without this feature, the macro is unavailable and you
must implement [`ResourceObject`] by hand.

```toml
[dependencies]
jsonapi_core = "0.1"   # derive is on
```

To opt out:

```toml
[dependencies]
jsonapi_core = { version = "0.1", default-features = false }
```

When opting out, you keep all the runtime types — `Document`, `Resource`,
`Registry`, `QueryBuilder`, `JsonApiMediaType`, etc. You only lose the macro.

## `atomic-ops`

Enables the `jsonapi_core::atomic` module — types for the
[Atomic Operations extension](./atomic-operations.md):

- `AtomicRequest`, `AtomicResponse`, `AtomicResult`
- `AtomicOperation`, `OperationTarget`, `OperationRef`
- `ATOMIC_EXT_URI` constant

```toml
[dependencies]
jsonapi_core = { version = "0.1", features = ["atomic-ops"] }
```

This is off by default because most JSON:API consumers don't need batching.

## Combining features

| Combo | What you get |
|-------|--------------|
| Default | Full type model + derive macro |
| `default-features = false` | Full type model, no macro |
| `features = ["atomic-ops"]` | Default plus the `atomic` module |
| `default-features = false, features = ["atomic-ops"]` | Type model + atomic, no macro |
