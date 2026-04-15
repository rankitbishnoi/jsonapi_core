use jsonapi_core::JsonApi;

#[derive(JsonApi)]
struct Bad {
    #[jsonapi(id)]
    id: String,
}

fn main() {}
