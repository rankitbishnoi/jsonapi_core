use jsonapi_core::JsonApi;

#[derive(JsonApi)]
#[jsonapi(type = "things")]
struct Thing(String, String);

fn main() {}
