use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "-invalid")]
struct Bad {
    #[jsonapi(id)]
    id: String,
}

fn main() {}
