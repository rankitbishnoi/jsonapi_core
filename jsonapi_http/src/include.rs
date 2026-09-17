//! Compound-document / `include` resolution.
//!
//! Given the primary resources, the client's requested `include` paths, and a
//! consumer-supplied [`IncludeResolver`], [`resolve_includes`] assembles the
//! deduped `included` array of a compound document — walking transitive paths
//! (e.g. `comments.author`) segment by segment and deduping by `(type, id)`.
//!
//! The library owns the walk / batching / dedup / assembly; **fetching stays the
//! consumer's job** (the resolver). This crate never touches a datastore and never
//! applies sort / filter / page — those are permanent non-goals.
//!
//! # Resolver interface
//!
//! The resolver is a **dumb batch loader keyed by identity** — it receives a
//! type and a deduped id list and returns the matching resources:
//!
//! ```
//! # use std::future::Future;
//! # use jsonapi_core::Resource;
//! trait IncludeResolver {
//!     type Error;
//!     fn load(&self, type_name: &str, ids: &[String])
//!         -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send;
//! }
//! ```
//!
//! [`resolve_includes`] extracts the linkage `(type, id)` refs itself, dedups
//! them, subtracts what is already loaded, and calls `load` **once per type per
//! level** — so the consumer cannot cause an N+1. The rejected alternative
//! (hand the resolver a path + parents and let it return related resources)
//! pushes linkage-walking and dedup onto every consumer and invites N+1.
//!
//! `Self::Error` is a generic associated type, not `JsonApiError`: this crate
//! must not depend on any web framework. An adapter maps the error at the edge
//! (e.g. `jsonapi_axum`'s `.or_json_api()`).
//!
//! # Sparse fieldsets
//!
//! This crate only *assembles* the `included` array. `fields[type]` filtering of
//! included resources happens later, at serialization
//! ([`json_api_response_filtered`](crate::json_api_response_filtered)); the two
//! compose without interaction here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;

use jsonapi_core::{RelationshipData, Resource, ResourceObject, ResourceRelationship};

/// Batch loader for compound-document resolution. See the [module docs](self).
///
/// Implement this over your datastore: given a `type_name` and a deduped list of
/// server-assigned `ids`, return the matching resources as dynamic
/// [`Resource`]s (via [`Resource::from_typed`](jsonapi_core::Resource::from_typed)).
/// Returning fewer resources than requested is fine — a ref that resolves to
/// nothing is simply dropped from the walk.
pub trait IncludeResolver {
    /// The consumer's load error. Mapped to a JSON:API error at the adapter edge.
    type Error;

    /// Load every resource of `type_name` whose id is in `ids`.
    ///
    /// Returns `impl Future + Send` (rather than a bare `async fn`) so the future
    /// composed by [`resolve_includes`] is `Send` and usable inside an axum
    /// handler.
    fn load(
        &self,
        type_name: &str,
        ids: &[String],
    ) -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send;
}

/// Assemble the deduped `included` array for a compound document.
///
/// For each path, starting from `primary`, walk segment by segment: read the
/// segment's linkage `(type, id)` refs from every resource at the current level,
/// batch-load the refs not already held (one `load` call per type), add the newly
/// loaded resources to the global `included` set (deduped by `(type, id)`), and
/// descend to those refs for the next segment.
///
/// Lid-only linkage refs (atomic-ops local ids, no server `id`) are skipped.
/// Primary resources are never re-loaded or emitted as an included duplicate, so
/// a cyclic linkage terminates after the finite path length. An empty / blank
/// path is a no-op; empty `paths` makes zero resolver calls.
///
/// The caller attaches the result while keeping the primary typed:
/// `DocumentBuilder::collection(primary).include_many(included).build()`.
pub async fn resolve_includes<R: IncludeResolver>(
    primary: &[Resource],
    paths: &[&str],
    resolver: &R,
) -> Result<Vec<Resource>, R::Error> {
    // `pool` owns every resource we hold (primary + loaded) for linkage reads and
    // dedup; `included_order` records emitted includes in first-seen order.
    let mut pool: HashMap<(String, String), Resource> = HashMap::new();
    let mut included_order: Vec<(String, String)> = Vec::new();

    // Seed the pool with primary resources: they anchor the first level and must
    // never be re-loaded (over-fetch) or emitted as an included duplicate.
    let mut primary_keys: Vec<(String, String)> = Vec::new();
    for resource in primary {
        if let Some(id) = resource.resource_id() {
            let key = (resource.r#type.clone(), id.to_string());
            pool.entry(key.clone()).or_insert_with(|| resource.clone());
            if !primary_keys.contains(&key) {
                primary_keys.push(key);
            }
        }
    }

    for path in paths {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }

        let mut current = primary_keys.clone();
        for segment in path.split('.') {
            // Deduped linkage refs from every resource at the current level.
            let mut refs: Vec<(String, String)> = Vec::new();
            let mut refs_seen: HashSet<(String, String)> = HashSet::new();
            for key in &current {
                let Some(resource) = pool.get(key) else {
                    continue;
                };
                let Some(rel) = resource.relationships.get(segment) else {
                    continue;
                };
                for reference in linkage_refs(rel) {
                    if refs_seen.insert(reference.clone()) {
                        refs.push(reference);
                    }
                }
            }

            // Batch-load only the refs we don't already hold — one call per type.
            let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (type_name, id) in &refs {
                if !pool.contains_key(&(type_name.clone(), id.clone())) {
                    missing
                        .entry(type_name.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
            for (type_name, ids) in missing {
                for resource in resolver.load(&type_name, &ids).await? {
                    let Some(id) = resource.resource_id().map(str::to_string) else {
                        continue;
                    };
                    let key = (resource.r#type.clone(), id);
                    if pool.contains_key(&key) {
                        continue; // resolver returned a duplicate; keep the first.
                    }
                    included_order.push(key.clone());
                    pool.insert(key, resource);
                }
            }

            // Descend: the next level is every ref we now hold (freshly loaded or
            // already pooled). Bounded by the finite path length, so cyclic
            // linkage always terminates.
            current = refs
                .into_iter()
                .filter(|key| pool.contains_key(key))
                .collect();
        }
    }

    Ok(included_order
        .into_iter()
        .map(|key| {
            pool.remove(&key)
                .expect("every included key was inserted into the pool")
        })
        .collect())
}

/// Extract server-assigned `(type, id)` linkage refs from a relationship,
/// skipping null linkage and lid-only refs (atomic-ops local ids).
fn linkage_refs(rel: &ResourceRelationship) -> Vec<(String, String)> {
    let Some(data) = &rel.data else {
        return Vec::new();
    };
    match data {
        RelationshipData::ToOne(Some(rid)) => rid
            .identity
            .as_id()
            .map(|id| vec![(rid.r#type.clone(), id.to_string())])
            .unwrap_or_default(),
        RelationshipData::ToOne(None) => Vec::new(),
        RelationshipData::ToMany(rids) => rids
            .iter()
            .filter_map(|rid| {
                rid.identity
                    .as_id()
                    .map(|id| (rid.r#type.clone(), id.to_string()))
            })
            .collect(),
        // `RelationshipData` is `#[non_exhaustive]`; ignore unknown future kinds.
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use jsonapi_core::model::{Identity, RelationshipData, ResourceIdentifier};

    /// A fake in-memory store: a `type -> id -> Resource` map plus a call counter
    /// to assert batching (no N+1). `load` returns a `Send` future that borrows
    /// nothing from `self`.
    #[derive(Default)]
    struct FakeStore {
        data: HashMap<(String, String), Resource>,
        calls: AtomicUsize,
        fail: bool,
    }

    #[derive(Debug, PartialEq)]
    struct LoadError(String);

    impl FakeStore {
        fn insert(&mut self, resource: Resource) {
            let key = (
                resource.r#type.clone(),
                resource.id.clone().expect("fixture needs an id"),
            );
            self.data.insert(key, resource);
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl IncludeResolver for FakeStore {
        type Error = LoadError;

        fn load(
            &self,
            type_name: &str,
            ids: &[String],
        ) -> impl Future<Output = Result<Vec<Resource>, Self::Error>> + Send {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let result = if self.fail {
                Err(LoadError(format!("boom loading {type_name}")))
            } else {
                Ok(ids
                    .iter()
                    .filter_map(|id| self.data.get(&(type_name.to_string(), id.clone())).cloned())
                    .collect())
            };
            async move { result }
        }
    }

    fn resource(type_: &str, id: &str) -> Resource {
        Resource {
            r#type: type_.into(),
            id: Some(id.into()),
            lid: None,
            attributes: serde_json::json!({}),
            relationships: BTreeMap::new(),
            links: None,
            meta: None,
        }
    }

    fn to_one(rel_type: &str, id: &str) -> ResourceRelationship {
        ResourceRelationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            r#type: rel_type.into(),
            identity: Identity::Id(id.into()),
            meta: None,
        })))
    }

    fn to_one_lid(rel_type: &str, lid: &str) -> ResourceRelationship {
        ResourceRelationship::new(RelationshipData::ToOne(Some(ResourceIdentifier {
            r#type: rel_type.into(),
            identity: Identity::Lid(lid.into()),
            meta: None,
        })))
    }

    fn to_many(rel_type: &str, ids: &[&str]) -> ResourceRelationship {
        ResourceRelationship::new(RelationshipData::ToMany(
            ids.iter()
                .map(|id| ResourceIdentifier {
                    r#type: rel_type.into(),
                    identity: Identity::Id((*id).into()),
                    meta: None,
                })
                .collect(),
        ))
    }

    fn keys(included: &[Resource]) -> Vec<(String, String)> {
        included
            .iter()
            .map(|r| (r.r#type.clone(), r.id.clone().unwrap()))
            .collect()
    }

    #[test]
    fn empty_include_makes_no_resolver_calls() {
        pollster::block_on(async {
            let store = FakeStore::default();
            let primary = vec![resource("articles", "1")];
            let included = resolve_includes(&primary, &[], &store).await.unwrap();
            assert!(included.is_empty());
            assert_eq!(store.calls(), 0);
        });
    }

    #[test]
    fn single_level_shared_author_appears_once() {
        pollster::block_on(async {
            let mut store = FakeStore::default();
            store.insert(resource("people", "9"));

            let mut a1 = resource("articles", "1");
            a1.relationships
                .insert("author".into(), to_one("people", "9"));
            let mut a2 = resource("articles", "2");
            a2.relationships
                .insert("author".into(), to_one("people", "9"));

            let included = resolve_includes(&[a1, a2], &["author"], &store)
                .await
                .unwrap();

            assert_eq!(keys(&included), vec![("people".into(), "9".into())]);
            // One batched load for the `people` type, not one per article.
            assert_eq!(store.calls(), 1);
        });
    }

    #[test]
    fn transitive_path_loads_and_dedups_each_level() {
        pollster::block_on(async {
            let mut store = FakeStore::default();
            // Two comments share one author.
            let mut c1 = resource("comments", "5");
            c1.relationships
                .insert("author".into(), to_one("people", "9"));
            let mut c2 = resource("comments", "6");
            c2.relationships
                .insert("author".into(), to_one("people", "9"));
            store.insert(c1);
            store.insert(c2);
            store.insert(resource("people", "9"));

            let mut article = resource("articles", "1");
            article
                .relationships
                .insert("comments".into(), to_many("comments", &["5", "6"]));

            let included = resolve_includes(&[article], &["comments.author"], &store)
                .await
                .unwrap();

            let mut got = keys(&included);
            got.sort();
            assert_eq!(
                got,
                vec![
                    ("comments".into(), "5".into()),
                    ("comments".into(), "6".into()),
                    ("people".into(), "9".into()),
                ]
            );
            // One load per level: `comments`, then `people` (deduped) — no N+1.
            assert_eq!(store.calls(), 2);
        });
    }

    #[test]
    fn lid_only_linkage_ref_is_skipped() {
        pollster::block_on(async {
            let store = FakeStore::default();
            let mut article = resource("articles", "1");
            article
                .relationships
                .insert("author".into(), to_one_lid("people", "local-1"));

            let included = resolve_includes(&[article], &["author"], &store)
                .await
                .unwrap();

            assert!(included.is_empty());
            // Nothing resolvable → no load call.
            assert_eq!(store.calls(), 0);
        });
    }

    #[test]
    fn cyclic_linkage_terminates_without_duplicates() {
        pollster::block_on(async {
            // a (primary) -> b -> a : a self-referential cycle through the store.
            let mut store = FakeStore::default();
            let mut b = resource("nodes", "b");
            b.relationships.insert("next".into(), to_one("nodes", "a"));
            store.insert(b);
            let mut a_in_store = resource("nodes", "a");
            a_in_store
                .relationships
                .insert("next".into(), to_one("nodes", "b"));
            store.insert(a_in_store);

            let mut a = resource("nodes", "a");
            a.relationships.insert("next".into(), to_one("nodes", "b"));

            // A deep path that would loop forever without the seen-set guard.
            let included = resolve_includes(&[a], &["next.next.next.next"], &store)
                .await
                .unwrap();

            // `b` is loaded once; `a` is primary so it is never emitted.
            assert_eq!(keys(&included), vec![("nodes".into(), "b".into())]);
        });
    }

    #[test]
    fn resolver_error_propagates_unchanged() {
        pollster::block_on(async {
            let store = FakeStore {
                fail: true,
                ..FakeStore::default()
            };
            let mut article = resource("articles", "1");
            article
                .relationships
                .insert("author".into(), to_one("people", "9"));

            let err = resolve_includes(&[article], &["author"], &store)
                .await
                .unwrap_err();
            assert_eq!(err, LoadError("boom loading people".into()));
        });
    }
}
