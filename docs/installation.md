# Installation

Add `jsonapi_core` to your `Cargo.toml` with `cargo add`:

```sh
cargo add jsonapi_core
```

Or edit `Cargo.toml` directly:

```toml
[dependencies]
jsonapi_core = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Minimum Supported Rust Version

`jsonapi_core` requires Rust **1.88** or later and is built on the **2024 edition**.

## Feature flags

| Feature | Default | What it does |
|---------|---------|--------------|
| `derive` | yes | Re-exports `#[derive(JsonApi)]` from `jsonapi_core_derive`. |
| `atomic-ops` | no | Enables the `jsonapi_core::atomic` module — types for the JSON:API Atomic Operations extension. |

To opt out of the derive macro (e.g. when implementing `ResourceObject` by hand):

```toml
[dependencies]
jsonapi_core = { version = "0.1", default-features = false }
```

To turn on Atomic Operations:

```toml
[dependencies]
jsonapi_core = { version = "0.1", features = ["atomic-ops"] }
```

See the [Feature Flags](./feature-flags.md) reference chapter for details on what
each flag toggles in the public API.

## Verifying the install

A minimal smoke test:

```rust
use jsonapi_core::{Document, Resource};

fn main() {
    let json = r#"{"data": {"type": "widgets", "id": "1", "attributes": {"color": "red"}}}"#;
    let doc: Document<Resource> = serde_json::from_str(json).unwrap();
    println!("{doc:#?}");
}
```

If this compiles and prints a parsed `Document`, you're good to go.
