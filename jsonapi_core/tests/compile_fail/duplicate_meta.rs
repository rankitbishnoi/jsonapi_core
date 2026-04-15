use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
struct Bad {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(meta)]
    meta1: Option<jsonapi_core::Meta>,
    #[jsonapi(meta)]
    meta2: Option<jsonapi_core::Meta>,
}

fn main() {}
