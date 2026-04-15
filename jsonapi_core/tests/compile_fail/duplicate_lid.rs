use jsonapi_core::JsonApi;

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "things")]
struct Thing {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(lid)]
    lid1: Option<String>,
    #[jsonapi(lid)]
    lid2: Option<String>,
}

fn main() {}
