//! Types sealed with `#[non_exhaustive]` for 1.0 must stay constructible from
//! outside the crate through constructors and `with_*` setters.

use jsonapi_core::{CaseConfig, CaseConvention, PageWindow, SortField};

#[test]
fn page_window_new_sets_all_fields() {
    let w = PageWindow::new(3, 20, 10);
    assert_eq!((w.number, w.offset, w.limit), (3, 20, 10));
}

#[test]
fn case_config_new_sets_member_case() {
    let c = CaseConfig::new(CaseConvention::CamelCase);
    assert_eq!(c.member_case, CaseConvention::CamelCase);
    assert_eq!(c.member_case.convert("first_name"), "firstName");
}

#[test]
fn sort_field_new_sets_field_and_direction() {
    let s = SortField::new("created", true);
    assert_eq!(s.field, "created");
    assert!(s.descending);
}

#[cfg(feature = "atomic-ops")]
mod atomic {
    use jsonapi_core::{
        AtomicOperation, AtomicRequest, AtomicResponse, AtomicResult, Identity, JsonApiObject,
        Links, Meta, OperationRef, OperationTarget, PrimaryData, Resource,
    };

    #[test]
    fn operation_ref_new_and_with_relationship() {
        let r = OperationRef::new("articles", Identity::Id("1".into())).with_relationship("author");
        assert_eq!(r.r#type, "articles");
        assert_eq!(r.identity, Identity::Id("1".into()));
        assert_eq!(r.relationship.as_deref(), Some("author"));
    }

    #[test]
    fn operation_target_reference_and_href() {
        let r = OperationRef::new("articles", Identity::Lid("a".into()));
        let t = OperationTarget::reference(r.clone());
        assert_eq!(t.r#ref, Some(r));
        assert_eq!(t.href, None);

        let h = OperationTarget::href("/articles/1");
        assert_eq!(h.href.as_deref(), Some("/articles/1"));
        assert_eq!(h.r#ref, None);
    }

    #[test]
    fn atomic_request_new_holds_operations() {
        let req = AtomicRequest::new(Vec::<AtomicOperation>::new());
        assert!(req.operations.is_empty());
    }

    #[test]
    fn atomic_result_setters() {
        let res = AtomicResult::default()
            .with_data(PrimaryData::Single(Box::new(Resource::new("articles"))))
            .with_meta(Meta::new())
            .with_links(Links::new());
        assert!(res.data.is_some());
        assert!(res.meta.is_some());
        assert!(res.links.is_some());
    }

    #[test]
    fn atomic_response_new_and_setters() {
        let resp = AtomicResponse::new(vec![AtomicResult::default()])
            .with_jsonapi(JsonApiObject::default())
            .with_meta(Meta::new())
            .with_links(Links::new());
        assert_eq!(resp.results.len(), 1);
        assert!(resp.jsonapi.is_some());
        assert!(resp.meta.is_some());
        assert!(resp.links.is_some());
    }
}
