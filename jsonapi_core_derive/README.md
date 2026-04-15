# jsonapi_core_derive

Procedural derive macro for the [`jsonapi_core`](https://crates.io/crates/jsonapi_core) crate.

This crate exposes only the `#[derive(JsonApi)]` macro. You almost certainly do
not want to depend on it directly — instead, depend on `jsonapi_core` with the
default `derive` feature enabled (which re-exports the macro):

```toml
[dependencies]
jsonapi_core = "0.1"
```

```rust
use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Article {
    #[jsonapi(id)]
    id: String,
    title: String,
}
```

See the [`jsonapi_core` documentation](https://docs.rs/jsonapi_core) and the
[derive macro reference](https://github.com/rankitbishnoi/jsonapi_core/blob/main/docs/derive-macro-reference.md)
for the full list of attributes and behaviour.

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
or [MIT license](http://opensource.org/licenses/MIT) at your option.
