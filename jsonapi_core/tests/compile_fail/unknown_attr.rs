use jsonapi_core::JsonApi;

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "things")]
struct Thing {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(frobnicate)]
    title: String,
}

fn main() {}
