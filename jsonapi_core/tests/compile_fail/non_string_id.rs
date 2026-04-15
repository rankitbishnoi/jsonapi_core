use jsonapi_core::JsonApi;

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "things")]
struct Thing {
    #[jsonapi(id)]
    id: u32,
    title: String,
}

fn main() {}
