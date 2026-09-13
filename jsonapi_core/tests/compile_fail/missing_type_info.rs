// A manual `ResourceObject` impl that omits `type_info()` must fail to compile
// once `type_info()` is a required method (no default body).
use jsonapi_core::ResourceObject;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Widget {
    id: String,
}

impl ResourceObject for Widget {
    fn resource_type(&self) -> &str {
        "widgets"
    }

    fn resource_id(&self) -> Option<&str> {
        Some(&self.id)
    }

    fn field_names() -> &'static [&'static str] {
        &["id"]
    }
    // NOTE: no `type_info()` — this is the point of the test.
}

fn main() {}
