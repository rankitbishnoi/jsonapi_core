//! JSON:API query strings: build them client-side with [`QueryBuilder`]; a server-side parser is added later.

mod builder;

pub use builder::QueryBuilder;
