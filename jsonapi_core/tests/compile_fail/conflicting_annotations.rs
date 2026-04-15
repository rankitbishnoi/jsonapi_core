use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
struct Bad {
    #[jsonapi(id, relationship)]
    id: String,
}

fn main() {}
