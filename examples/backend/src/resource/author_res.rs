use crate::domain::Author;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "authors", case = "camelCase")]
pub struct AuthorResource {
    #[jsonapi(id)]
    pub id: String,
    pub name: String,
    pub email: String,
    pub created_at: String,
}

impl From<Author> for AuthorResource {
    fn from(a: Author) -> Self {
        Self {
            id: a.id,
            name: a.name,
            email: a.email,
            created_at: a.created_at,
        }
    }
}
