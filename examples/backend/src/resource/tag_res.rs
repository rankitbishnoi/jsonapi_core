use crate::domain::Tag;

#[derive(Debug, Clone, jsonapi_core::JsonApi)]
#[jsonapi(type = "tags")]
pub struct TagResource {
    #[jsonapi(id)]
    pub id: String,
    pub name: String,
}

impl From<Tag> for TagResource {
    fn from(t: Tag) -> Self {
        Self {
            id: t.id,
            name: t.name,
        }
    }
}
