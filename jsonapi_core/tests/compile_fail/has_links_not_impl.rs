// Improvement #5 negative case: a resource without `#[jsonapi(links)]` must
// NOT implement `HasLinks` (the derive only emits the trait when the field
// is present). This compile-fail test pins that contract.

use jsonapi_core::JsonApi;
use jsonapi_core::model::HasLinks;

#[derive(JsonApi)]
#[jsonapi(type = "people")]
struct Person {
    #[jsonapi(id)]
    id: String,
    name: String,
}

fn requires_has_links<T: HasLinks>(_: &T) {}

fn main() {
    let p = Person {
        id: "1".into(),
        name: "Dan".into(),
    };
    requires_has_links(&p);
}
