use heck::{ToKebabCase, ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::Span;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Type};

use crate::validate;

/// Parsed struct-level attributes.
pub struct StructAttrs {
    pub type_name: String,
    pub case: CaseKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    CamelCase,
    SnakeCase,
    KebabCase,
    PascalCase,
    None,
}

impl CaseKind {
    pub fn from_str(s: &str, span: Span) -> syn::Result<Self> {
        match s {
            "camelCase" => Ok(CaseKind::CamelCase),
            "snake_case" => Ok(CaseKind::SnakeCase),
            "kebab-case" => Ok(CaseKind::KebabCase),
            "PascalCase" => Ok(CaseKind::PascalCase),
            "none" => Ok(CaseKind::None),
            _ => Err(syn::Error::new(
                span,
                format!(
                    "unknown case convention: \"{s}\", expected one of: \
                     camelCase, snake_case, kebab-case, PascalCase, none"
                ),
            )),
        }
    }

    pub fn convert(&self, name: &str) -> String {
        match self {
            CaseKind::CamelCase => name.to_lower_camel_case(),
            CaseKind::SnakeCase => name.to_snake_case(),
            CaseKind::KebabCase => name.to_kebab_case(),
            CaseKind::PascalCase => name.to_upper_camel_case(),
            CaseKind::None => name.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum FieldKind {
    Id,
    Lid,
    Attribute,
    Relationship,
    Meta,
    Links,
    Skip,
}

pub struct ParsedField {
    pub ident: Ident,
    pub ty: Type,
    pub kind: FieldKind,
    pub wire_name: Option<String>,
    pub aliases: Vec<String>,
    pub is_option: bool,
    pub is_vec: bool,
    pub rel_target_type: Option<String>,
}

pub fn parse(input: &DeriveInput) -> syn::Result<(StructAttrs, Vec<ParsedField>)> {
    let struct_attrs = parse_struct_attrs(input)?;
    let fields = parse_fields(input, &struct_attrs)?;
    validate_fields(&fields, input)?;
    Ok((struct_attrs, fields))
}

fn parse_struct_attrs(input: &DeriveInput) -> syn::Result<StructAttrs> {
    let mut type_name: Option<String> = None;
    let mut case = CaseKind::None;

    for attr in &input.attrs {
        if !attr.path().is_ident("jsonapi") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type") {
                let value: LitStr = meta.value()?.parse()?;
                let name = value.value();
                validate::validate_member_name(&name).map_err(|reason| {
                    syn::Error::new(
                        value.span(),
                        format!("invalid JSON:API type string: {reason}"),
                    )
                })?;
                type_name = Some(name);
                Ok(())
            } else if meta.path.is_ident("case") {
                let value: LitStr = meta.value()?.parse()?;
                case = CaseKind::from_str(&value.value(), value.span())?;
                Ok(())
            } else {
                Err(meta.error("expected `type` or `case`"))
            }
        })?;
    }

    let type_name = type_name.ok_or_else(|| {
        syn::Error::new(
            input.ident.span(),
            "#[jsonapi(type = \"...\")] is required on the struct",
        )
    })?;

    Ok(StructAttrs { type_name, case })
}

fn parse_fields(input: &DeriveInput, struct_attrs: &StructAttrs) -> syn::Result<Vec<ParsedField>> {
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "#[derive(JsonApi)] only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(
                input.ident.span(),
                "#[derive(JsonApi)] only supports structs",
            ));
        }
    };

    let mut parsed = Vec::new();

    for field in fields {
        let ident = field.ident.clone().unwrap();
        let ty = field.ty.clone();
        let is_option = is_option_type(&ty);
        let is_vec = is_vec_type(&ty);

        let mut kind = FieldKind::Attribute;
        let mut rename: Option<String> = None;
        let mut rel_target_type: Option<String> = None;

        for attr in &field.attrs {
            if !attr.path().is_ident("jsonapi") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("id") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Id;
                    Ok(())
                } else if meta.path.is_ident("lid") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Lid;
                    Ok(())
                } else if meta.path.is_ident("relationship") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Relationship;
                    Ok(())
                } else if meta.path.is_ident("meta") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Meta;
                    Ok(())
                } else if meta.path.is_ident("links") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Links;
                    Ok(())
                } else if meta.path.is_ident("skip") {
                    if !matches!(kind, FieldKind::Attribute) {
                        return Err(meta.error("conflicting field annotations; only one of id, lid, relationship, meta, links, skip is allowed"));
                    }
                    kind = FieldKind::Skip;
                    Ok(())
                } else if meta.path.is_ident("rename") {
                    let value: LitStr = meta.value()?.parse()?;
                    let name = value.value();
                    validate::validate_member_name(&name).map_err(|reason| {
                        syn::Error::new(value.span(), format!("invalid rename value: {reason}"))
                    })?;
                    rename = Some(name);
                    Ok(())
                } else if meta.path.is_ident("type") {
                    let value: LitStr = meta.value()?.parse()?;
                    rel_target_type = Some(value.value());
                    Ok(())
                } else {
                    Err(meta.error(
                        "expected one of: id, lid, relationship, meta, links, skip, rename, type",
                    ))
                }
            })?;
        }

        let (wire_name, aliases) = match &kind {
            FieldKind::Attribute | FieldKind::Relationship => {
                if let Some(ref renamed) = rename {
                    (Some(renamed.clone()), vec![])
                } else {
                    let field_name = ident.to_string();
                    let wire = struct_attrs.case.convert(&field_name);
                    let aliases = generate_aliases(&field_name, struct_attrs.case);
                    (Some(wire), aliases)
                }
            }
            _ => (None, vec![]),
        };

        parsed.push(ParsedField {
            ident,
            ty,
            kind,
            wire_name,
            aliases,
            is_option,
            is_vec,
            rel_target_type,
        });
    }

    Ok(parsed)
}

fn generate_aliases(field_name: &str, output_case: CaseKind) -> Vec<String> {
    let output = output_case.convert(field_name);

    let all_cases = [
        CaseKind::CamelCase,
        CaseKind::SnakeCase,
        CaseKind::KebabCase,
        CaseKind::PascalCase,
        CaseKind::None,
    ];

    let mut aliases = vec![output.clone()];

    for case in &all_cases {
        let converted = case.convert(field_name);
        if !aliases.contains(&converted) {
            aliases.push(converted);
        }
    }

    if aliases.len() == 1 {
        return vec![];
    }

    aliases
}

fn validate_fields(fields: &[ParsedField], input: &DeriveInput) -> syn::Result<()> {
    let mut id_span: Option<Span> = None;
    let mut lid_span: Option<Span> = None;
    let mut meta_span: Option<Span> = None;
    let mut links_span: Option<Span> = None;

    for field in fields {
        match field.kind {
            FieldKind::Id => {
                if let Some(first) = id_span {
                    let mut err =
                        syn::Error::new(field.ident.span(), "duplicate #[jsonapi(id)] field");
                    err.combine(syn::Error::new(first, "first #[jsonapi(id)] here"));
                    return Err(err);
                }
                id_span = Some(field.ident.span());
            }
            FieldKind::Lid => {
                if let Some(first) = lid_span {
                    let mut err =
                        syn::Error::new(field.ident.span(), "duplicate #[jsonapi(lid)] field");
                    err.combine(syn::Error::new(first, "first #[jsonapi(lid)] here"));
                    return Err(err);
                }
                lid_span = Some(field.ident.span());
            }
            FieldKind::Meta => {
                if let Some(first) = meta_span {
                    let mut err =
                        syn::Error::new(field.ident.span(), "duplicate #[jsonapi(meta)] field");
                    err.combine(syn::Error::new(first, "first #[jsonapi(meta)] here"));
                    return Err(err);
                }
                meta_span = Some(field.ident.span());
            }
            FieldKind::Links => {
                if let Some(first) = links_span {
                    let mut err =
                        syn::Error::new(field.ident.span(), "duplicate #[jsonapi(links)] field");
                    err.combine(syn::Error::new(first, "first #[jsonapi(links)] here"));
                    return Err(err);
                }
                links_span = Some(field.ident.span());
            }
            _ => {}
        }

        if matches!(field.kind, FieldKind::Id)
            && !is_string_type(&field.ty)
            && !is_option_string_type(&field.ty)
        {
            return Err(syn::Error::new(
                field.ident.span(),
                "#[jsonapi(id)] field must be String or Option<String>",
            ));
        }
        if field.rel_target_type.is_some() && !matches!(field.kind, FieldKind::Relationship) {
            return Err(syn::Error::new(
                field.ident.span(),
                "`type = \"...\"` is only valid on #[jsonapi(relationship)] fields",
            ));
        }
    }

    if id_span.is_none() {
        return Err(syn::Error::new(
            input.ident.span(),
            "exactly one #[jsonapi(id)] field is required",
        ));
    }

    Ok(())
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        // Only match single-segment paths (e.g. `Option<T>`, not `my::Option<T>`)
        // Note: type aliases like `type MyOpt<T> = Option<T>` are not supported.
        type_path.qself.is_none()
            && type_path.path.segments.len() == 1
            && type_path.path.segments[0].ident == "Option"
    } else {
        false
    }
}

fn is_vec_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path.qself.is_none()
            && type_path.path.segments.len() == 1
            && type_path.path.segments[0].ident == "Vec"
    } else {
        false
    }
}

fn is_string_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path.qself.is_none()
            && type_path.path.segments.len() == 1
            && type_path.path.segments[0].ident == "String"
    } else {
        false
    }
}

fn is_option_string_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && type_path.qself.is_none()
        && type_path.path.segments.len() == 1
        && type_path.path.segments[0].ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &type_path.path.segments[0].arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return is_string_type(inner);
    }
    false
}
