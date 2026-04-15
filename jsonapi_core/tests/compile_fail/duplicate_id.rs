use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
struct Bad {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(id)]
    other_id: String,
}

fn main() {}
