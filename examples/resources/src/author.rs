#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "authors", case = "camelCase")]
pub struct AuthorResource {
    #[jsonapi(id)]
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
}
