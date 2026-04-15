use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
struct Bad {
    name: String,
}

fn main() {}
