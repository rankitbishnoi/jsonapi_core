#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "tags")]
pub struct TagResource {
    #[jsonapi(id)]
    pub id: String,
    pub name: String,
}
