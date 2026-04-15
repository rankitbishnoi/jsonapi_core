use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "articles")]
struct Bad {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(type = "people")]
    title: String,
}

fn main() {}
