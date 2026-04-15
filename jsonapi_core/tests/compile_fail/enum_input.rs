use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
enum Thing {
    A,
    B,
}

fn main() {}
