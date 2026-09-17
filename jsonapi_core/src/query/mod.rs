//! JSON:API query strings: build them client-side with [`QueryBuilder`] and
//! parse them server-side with [`Query`].

mod builder;
mod parse;

pub use builder::QueryBuilder;
pub use parse::{Query, SortField};
