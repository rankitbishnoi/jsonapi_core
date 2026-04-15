//! Atomic operations request types.

use std::collections::HashSet;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Identity, PrimaryData, Resource};

/// Request envelope carrying an ordered list of atomic operations.
///
/// Wire form: `{"atomic:operations": [...]}`. Execution order is significant —
/// later operations may reference `lid` values introduced by earlier `add` ops.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AtomicRequest {
    /// Ordered list of operations to execute atomically.
    ///
    /// The array must be present on the wire; an absent `atomic:operations`
    /// key is a deserialization error.
    #[serde(rename = "atomic:operations")]
    pub operations: Vec<AtomicOperation>,
}

/// A single atomic operation.
///
/// Discriminated on the wire by the `op` field, which takes the lowercase
/// values `"add"`, `"update"`, or `"remove"`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum AtomicOperation {
    /// Create a new resource, or add to a to-many relationship.
    Add {
        /// Where the operation applies. Often empty for top-level resource creation.
        #[serde(flatten)]
        target: OperationTarget,
        /// Resource(s) being created, or identifier(s) being added to a relationship.
        data: PrimaryData<Resource>,
    },
    /// Update an existing resource or relationship.
    Update {
        /// Target of the update.
        #[serde(flatten)]
        target: OperationTarget,
        /// Replacement resource or relationship linkage.
        data: PrimaryData<Resource>,
    },
    /// Delete a resource or remove linkage from a relationship.
    Remove {
        /// Target of the removal.
        #[serde(flatten)]
        target: OperationTarget,
    },
}

/// Where an atomic operation applies.
///
/// Three legal wire states are representable:
///
/// | `ref`    | `href`   | Meaning                                               |
/// |----------|----------|-------------------------------------------------------|
/// | `Some`   | `None`   | Structured pointer (type + id/lid + optional rel).    |
/// | `None`   | `Some`   | URL pointer (alternative to `ref`).                   |
/// | `None`   | `None`   | No target — e.g. `add` creating a top-level resource. |
///
/// Having both set is spec-illegal. Use [`OperationTarget::is_valid`] to check,
/// or [`AtomicRequest::validate_lid_refs`](crate::AtomicRequest::validate_lid_refs)
/// which also flags this case along with `lid` reference errors.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OperationTarget {
    /// Structured reference to the target resource.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<OperationRef>,

    /// URL pointer to the target (alternative to `ref`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

impl OperationTarget {
    /// Returns `false` if both `ref` and `href` are set (spec violation).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !(self.r#ref.is_some() && self.href.is_some())
    }
}

/// Structured reference to the target of an atomic operation.
///
/// Reuses [`Identity`](crate::Identity) for id/lid consistency with
/// [`ResourceIdentifier`](crate::ResourceIdentifier). An optional
/// `relationship` name narrows the operation to a specific relationship
/// of the referenced resource.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationRef {
    /// JSON:API type string.
    pub type_: String,
    /// Server-assigned id or client-local lid.
    pub identity: Identity,
    /// Relationship name, if this op targets a relationship rather than the resource itself.
    pub relationship: Option<String>,
}

#[derive(Serialize)]
struct OperationRefSerRepr<'a> {
    #[serde(rename = "type")]
    type_: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lid: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relationship: Option<&'a str>,
}

#[derive(Deserialize)]
struct OperationRefDeRepr {
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    lid: Option<String>,
    #[serde(default)]
    relationship: Option<String>,
}

impl Serialize for OperationRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (id, lid) = match &self.identity {
            Identity::Id(id) => (Some(id.as_str()), None),
            Identity::Lid(lid) => (None, Some(lid.as_str())),
        };
        OperationRefSerRepr {
            type_: &self.type_,
            id,
            lid,
            relationship: self.relationship.as_deref(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OperationRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = OperationRefDeRepr::deserialize(deserializer)?;
        let identity = match (repr.id, repr.lid) {
            (Some(_), Some(_)) => {
                return Err(de::Error::custom(
                    "operation ref must not have both `id` and `lid`",
                ));
            }
            (Some(id), None) => Identity::Id(id),
            (None, Some(lid)) => Identity::Lid(lid),
            (None, None) => return Err(de::Error::custom("operation ref must have `id` or `lid`")),
        };
        Ok(OperationRef {
            type_: repr.type_,
            identity,
            relationship: repr.relationship,
        })
    }
}

impl AtomicRequest {
    /// Check that every `lid` referenced by an operation is introduced by a
    /// strictly earlier `add` operation, that no `lid` is introduced twice,
    /// and that no operation targets both `ref` and `href`.
    ///
    /// Returns `Error::InvalidAtomicOperation` at the first offending index.
    ///
    /// Serialization and deserialization of `AtomicRequest` remain infallible
    /// with respect to `lid` semantics; pre-flight validation is opt-in.
    pub fn validate_lid_refs(&self) -> crate::Result<()> {
        let mut introduced: HashSet<String> = HashSet::new();

        for (index, op) in self.operations.iter().enumerate() {
            let target = op.target();

            if !target.is_valid() {
                return Err(crate::Error::InvalidAtomicOperation {
                    index,
                    reason: "operation target must not have both `ref` and `href`".into(),
                });
            }

            if let Some(r) = &target.r#ref
                && let Identity::Lid(lid) = &r.identity
                && !introduced.contains(lid)
            {
                return Err(crate::Error::InvalidAtomicOperation {
                    index,
                    reason: format!("ref.lid `{lid}` not introduced by an earlier `add`"),
                });
            }

            if let AtomicOperation::Add { data, .. } = op {
                for resource in primary_data_resources(data) {
                    if let Some(lid) = &resource.lid
                        && !introduced.insert(lid.clone())
                    {
                        return Err(crate::Error::InvalidAtomicOperation {
                            index,
                            reason: format!("duplicate `lid` introduction: `{lid}`"),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

impl AtomicOperation {
    /// Borrow the operation's target (present on all variants).
    fn target(&self) -> &OperationTarget {
        match self {
            AtomicOperation::Add { target, .. }
            | AtomicOperation::Update { target, .. }
            | AtomicOperation::Remove { target } => target,
        }
    }
}

/// Iterate over the resources inside a `PrimaryData<Resource>`, skipping `Null`.
fn primary_data_resources(
    data: &PrimaryData<Resource>,
) -> Box<dyn Iterator<Item = &Resource> + '_> {
    match data {
        PrimaryData::Null => Box::new(std::iter::empty()),
        PrimaryData::Single(boxed) => Box::new(std::iter::once(boxed.as_ref())),
        PrimaryData::Many(vec) => Box::new(vec.iter()),
    }
}

#[cfg(test)]
mod operation_ref_tests {
    use super::*;
    use crate::Identity;

    #[test]
    fn serializes_with_id() {
        let r = OperationRef {
            type_: "articles".into(),
            identity: Identity::Id("1".into()),
            relationship: None,
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"type":"articles","id":"1"}"#
        );
    }

    #[test]
    fn serializes_with_lid() {
        let r = OperationRef {
            type_: "articles".into(),
            identity: Identity::Lid("local-1".into()),
            relationship: None,
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"type":"articles","lid":"local-1"}"#
        );
    }

    #[test]
    fn serializes_with_relationship() {
        let r = OperationRef {
            type_: "articles".into(),
            identity: Identity::Id("1".into()),
            relationship: Some("comments".into()),
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"{"type":"articles","id":"1","relationship":"comments"}"#
        );
    }

    #[test]
    fn deserializes_with_id() {
        let r: OperationRef = serde_json::from_str(r#"{"type":"articles","id":"1"}"#).unwrap();
        assert_eq!(r.type_, "articles");
        assert_eq!(r.identity, Identity::Id("1".into()));
        assert_eq!(r.relationship, None);
    }

    #[test]
    fn deserializes_with_lid() {
        let r: OperationRef =
            serde_json::from_str(r#"{"type":"articles","lid":"local-1"}"#).unwrap();
        assert_eq!(r.identity, Identity::Lid("local-1".into()));
    }

    #[test]
    fn deserializes_with_relationship() {
        let r: OperationRef =
            serde_json::from_str(r#"{"type":"articles","id":"1","relationship":"comments"}"#)
                .unwrap();
        assert_eq!(r.relationship, Some("comments".into()));
    }

    #[test]
    fn rejects_missing_identity() {
        let result: std::result::Result<OperationRef, _> =
            serde_json::from_str(r#"{"type":"articles"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_both_id_and_lid() {
        let result: std::result::Result<OperationRef, _> =
            serde_json::from_str(r#"{"type":"articles","id":"1","lid":"local-1"}"#);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod operation_target_tests {
    use super::*;
    use crate::Identity;

    fn ref_only() -> OperationTarget {
        OperationTarget {
            r#ref: Some(OperationRef {
                type_: "articles".into(),
                identity: Identity::Id("1".into()),
                relationship: None,
            }),
            href: None,
        }
    }

    fn href_only() -> OperationTarget {
        OperationTarget {
            r#ref: None,
            href: Some("/articles/1".into()),
        }
    }

    fn neither() -> OperationTarget {
        OperationTarget {
            r#ref: None,
            href: None,
        }
    }

    fn both() -> OperationTarget {
        OperationTarget {
            r#ref: Some(OperationRef {
                type_: "articles".into(),
                identity: Identity::Id("1".into()),
                relationship: None,
            }),
            href: Some("/articles/1".into()),
        }
    }

    #[test]
    fn is_valid_ref_only() {
        assert!(ref_only().is_valid());
    }

    #[test]
    fn is_valid_href_only() {
        assert!(href_only().is_valid());
    }

    #[test]
    fn is_valid_neither() {
        assert!(neither().is_valid());
    }

    #[test]
    fn is_invalid_when_both_set() {
        assert!(!both().is_valid());
    }

    #[test]
    fn serializes_ref_only() {
        let json = serde_json::to_value(ref_only()).unwrap();
        assert_eq!(json["ref"]["type"], "articles");
        assert_eq!(json["ref"]["id"], "1");
        assert!(json.get("href").is_none());
    }

    #[test]
    fn serializes_href_only() {
        let json = serde_json::to_value(href_only()).unwrap();
        assert_eq!(json["href"], "/articles/1");
        assert!(json.get("ref").is_none());
    }

    #[test]
    fn serializes_neither_as_empty_object() {
        let json = serde_json::to_string(&neither()).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn deserializes_ref_only() {
        let t: OperationTarget =
            serde_json::from_str(r#"{"ref":{"type":"articles","id":"1"}}"#).unwrap();
        assert!(t.r#ref.is_some());
        assert!(t.href.is_none());
    }

    #[test]
    fn deserializes_href_only() {
        let t: OperationTarget = serde_json::from_str(r#"{"href":"/articles/1"}"#).unwrap();
        assert!(t.r#ref.is_none());
        assert_eq!(t.href.as_deref(), Some("/articles/1"));
    }

    #[test]
    fn deserializes_empty_object() {
        let t: OperationTarget = serde_json::from_str("{}").unwrap();
        assert!(t.r#ref.is_none());
        assert!(t.href.is_none());
    }
}

#[cfg(test)]
mod atomic_operation_tests {
    use super::*;
    use crate::{Identity, PrimaryData, Resource};

    fn sample_ref() -> OperationRef {
        OperationRef {
            type_: "articles".into(),
            identity: Identity::Id("1".into()),
            relationship: None,
        }
    }

    fn sample_resource() -> Resource {
        Resource {
            type_: "articles".into(),
            id: None,
            lid: Some("local-1".into()),
            attributes: serde_json::json!({"title": "Hello"}),
            relationships: std::collections::BTreeMap::new(),
            links: None,
            meta: None,
        }
    }

    #[test]
    fn add_op_serializes_with_lowercase_op() {
        let op = AtomicOperation::Add {
            target: OperationTarget::default(),
            data: PrimaryData::Single(Box::new(sample_resource())),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "add");
    }

    #[test]
    fn update_op_serializes_with_lowercase_op() {
        let op = AtomicOperation::Update {
            target: OperationTarget {
                r#ref: Some(sample_ref()),
                href: None,
            },
            data: PrimaryData::Single(Box::new(sample_resource())),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "update");
        assert_eq!(json["ref"]["type"], "articles");
    }

    #[test]
    fn remove_op_has_no_data_field() {
        let op = AtomicOperation::Remove {
            target: OperationTarget {
                r#ref: Some(sample_ref()),
                href: None,
            },
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "remove");
        assert_eq!(json["ref"]["id"], "1");
        assert!(json.get("data").is_none());
    }

    #[test]
    fn deserializes_add() {
        let json = r#"{"op":"add","data":{"type":"articles","lid":"l1","attributes":{}}}"#;
        let op: AtomicOperation = serde_json::from_str(json).unwrap();
        match op {
            AtomicOperation::Add { target, data } => {
                assert!(target.r#ref.is_none() && target.href.is_none());
                assert!(matches!(data, PrimaryData::Single(_)));
            }
            _ => panic!("expected Add variant"),
        }
    }

    #[test]
    fn deserializes_update_with_ref() {
        let json = r#"{"op":"update","ref":{"type":"articles","id":"1"},"data":{"type":"articles","id":"1","attributes":{"title":"New"}}}"#;
        let op: AtomicOperation = serde_json::from_str(json).unwrap();
        match op {
            AtomicOperation::Update { target, .. } => {
                assert_eq!(target.r#ref.unwrap().type_, "articles");
            }
            _ => panic!("expected Update variant"),
        }
    }

    #[test]
    fn deserializes_remove() {
        let json = r#"{"op":"remove","ref":{"type":"articles","id":"1"}}"#;
        let op: AtomicOperation = serde_json::from_str(json).unwrap();
        assert!(matches!(op, AtomicOperation::Remove { .. }));
    }

    #[test]
    fn rejects_unknown_op() {
        let json = r#"{"op":"frobnicate","ref":{"type":"articles","id":"1"}}"#;
        let result: std::result::Result<AtomicOperation, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn add_many_to_relationship_round_trip() {
        let op = AtomicOperation::Add {
            target: OperationTarget {
                r#ref: Some(OperationRef {
                    type_: "articles".into(),
                    identity: Identity::Id("1".into()),
                    relationship: Some("tags".into()),
                }),
                href: None,
            },
            data: PrimaryData::Many(vec![Resource {
                type_: "tags".into(),
                id: Some("5".into()),
                lid: None,
                attributes: serde_json::json!({}),
                relationships: std::collections::BTreeMap::new(),
                links: None,
                meta: None,
            }]),
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "add");
        assert_eq!(json["ref"]["relationship"], "tags");
        assert_eq!(json["data"][0]["type"], "tags");

        let round: AtomicOperation = serde_json::from_value(json).unwrap();
        assert_eq!(round, op);
    }
}

#[cfg(test)]
mod atomic_request_tests {
    use super::*;
    use crate::{Identity, PrimaryData, Resource};

    #[test]
    fn empty_request_round_trips() {
        let req = AtomicRequest::default();
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"atomic:operations":[]}"#);
        let round: AtomicRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(round, req);
    }

    #[test]
    fn multi_op_request_round_trips() {
        let req = AtomicRequest {
            operations: vec![
                AtomicOperation::Add {
                    target: OperationTarget::default(),
                    data: PrimaryData::Single(Box::new(Resource {
                        type_: "people".into(),
                        id: None,
                        lid: Some("p1".into()),
                        attributes: serde_json::json!({"name": "Dan"}),
                        relationships: std::collections::BTreeMap::new(),
                        links: None,
                        meta: None,
                    })),
                },
                AtomicOperation::Remove {
                    target: OperationTarget {
                        r#ref: Some(OperationRef {
                            type_: "articles".into(),
                            identity: Identity::Id("9".into()),
                            relationship: None,
                        }),
                        href: None,
                    },
                },
            ],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["atomic:operations"].as_array().unwrap().len(), 2);
        assert_eq!(json["atomic:operations"][0]["op"], "add");
        assert_eq!(json["atomic:operations"][1]["op"], "remove");

        let round: AtomicRequest = serde_json::from_value(json).unwrap();
        assert_eq!(round, req);
    }

    #[test]
    fn missing_operations_field_is_error() {
        // Spec requires `atomic:operations`. Missing it must fail.
        let result: std::result::Result<AtomicRequest, _> = serde_json::from_str("{}");
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod validate_lid_refs_tests {
    use super::*;
    use crate::{Error, Identity, PrimaryData, Resource};

    fn add_with_lid(lid: &str) -> AtomicOperation {
        AtomicOperation::Add {
            target: OperationTarget::default(),
            data: PrimaryData::Single(Box::new(Resource {
                type_: "people".into(),
                id: None,
                lid: Some(lid.into()),
                attributes: serde_json::json!({}),
                relationships: std::collections::BTreeMap::new(),
                links: None,
                meta: None,
            })),
        }
    }

    fn update_ref_lid(lid: &str) -> AtomicOperation {
        AtomicOperation::Update {
            target: OperationTarget {
                r#ref: Some(OperationRef {
                    type_: "people".into(),
                    identity: Identity::Lid(lid.into()),
                    relationship: None,
                }),
                href: None,
            },
            data: PrimaryData::Single(Box::new(Resource {
                type_: "people".into(),
                id: None,
                lid: Some(lid.into()),
                attributes: serde_json::json!({"name": "X"}),
                relationships: std::collections::BTreeMap::new(),
                links: None,
                meta: None,
            })),
        }
    }

    #[test]
    fn happy_path_introduce_then_reference() {
        let req = AtomicRequest {
            operations: vec![add_with_lid("p1"), update_ref_lid("p1")],
        };
        assert!(req.validate_lid_refs().is_ok());
    }

    #[test]
    fn empty_request_is_valid() {
        let req = AtomicRequest::default();
        assert!(req.validate_lid_refs().is_ok());
    }

    #[test]
    fn reference_before_introduction_errors() {
        let req = AtomicRequest {
            operations: vec![update_ref_lid("p1"), add_with_lid("p1")],
        };
        match req.validate_lid_refs().unwrap_err() {
            Error::InvalidAtomicOperation { index, reason } => {
                assert_eq!(index, 0);
                assert!(reason.contains("p1"));
            }
            other => panic!("expected InvalidAtomicOperation, got {other:?}"),
        }
    }

    #[test]
    fn unresolvable_lid_errors() {
        let req = AtomicRequest {
            operations: vec![update_ref_lid("ghost")],
        };
        match req.validate_lid_refs().unwrap_err() {
            Error::InvalidAtomicOperation { index, .. } => assert_eq!(index, 0),
            other => panic!("expected InvalidAtomicOperation, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_lid_introduction_errors() {
        let req = AtomicRequest {
            operations: vec![add_with_lid("p1"), add_with_lid("p1")],
        };
        match req.validate_lid_refs().unwrap_err() {
            Error::InvalidAtomicOperation { index, reason } => {
                assert_eq!(index, 1);
                assert!(reason.contains("duplicate"));
            }
            other => panic!("expected InvalidAtomicOperation, got {other:?}"),
        }
    }

    #[test]
    fn target_with_both_ref_and_href_errors() {
        let req = AtomicRequest {
            operations: vec![AtomicOperation::Remove {
                target: OperationTarget {
                    r#ref: Some(OperationRef {
                        type_: "articles".into(),
                        identity: Identity::Id("1".into()),
                        relationship: None,
                    }),
                    href: Some("/articles/1".into()),
                },
            }],
        };
        match req.validate_lid_refs().unwrap_err() {
            Error::InvalidAtomicOperation { index, reason } => {
                assert_eq!(index, 0);
                assert!(reason.contains("ref") && reason.contains("href"));
            }
            other => panic!("expected InvalidAtomicOperation, got {other:?}"),
        }
    }
}
