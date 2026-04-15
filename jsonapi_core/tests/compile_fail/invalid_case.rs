use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things", case = "SCREAMING_CASE")]
struct Bad {
    #[jsonapi(id)]
    id: String,
}

fn main() {}
