use jsonapi_core::JsonApi;

#[derive(Debug, Clone, PartialEq, JsonApi)]
#[jsonapi(type = "things")]
struct Thing {
    #[jsonapi(id)]
    id: String,
    #[jsonapi(links)]
    links1: Option<jsonapi_core::Links>,
    #[jsonapi(links)]
    links2: Option<jsonapi_core::Links>,
}

fn main() {}
