use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::parse::{FieldKind, ParsedField, StructAttrs};

/// Generate all three impls: ResourceObject, Serialize, Deserialize.
pub fn generate(
    name: &syn::Ident,
    struct_attrs: &StructAttrs,
    fields: &[ParsedField],
) -> TokenStream {
    let resource_object_impl = gen_resource_object(name, struct_attrs, fields);
    let serialize_impl = gen_serialize(name, struct_attrs, fields);
    let deserialize_impl = gen_deserialize(name, struct_attrs, fields);

    quote! {
        #resource_object_impl
        #serialize_impl
        #deserialize_impl
    }
}

fn gen_resource_object(
    name: &syn::Ident,
    struct_attrs: &StructAttrs,
    fields: &[ParsedField],
) -> TokenStream {
    let type_name = &struct_attrs.type_name;

    // Find id field
    let id_field = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Id))
        .unwrap();
    let id_ident = &id_field.ident;
    let id_expr = if id_field.is_option {
        quote! { self.#id_ident.as_deref() }
    } else {
        quote! { ::core::option::Option::Some(self.#id_ident.as_str()) }
    };

    // Find lid field (optional)
    let lid_expr = if let Some(lid_field) = fields.iter().find(|f| matches!(f.kind, FieldKind::Lid))
    {
        let lid_ident = &lid_field.ident;
        quote! { self.#lid_ident.as_deref() }
    } else {
        quote! { ::core::option::Option::None }
    };

    // Collect wire names for field_names() — attributes + relationships
    let field_name_strs: Vec<&str> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Attribute | FieldKind::Relationship))
        .filter_map(|f| f.wire_name.as_deref())
        .collect();

    // Collect relationship (wire_name, target_type) pairs for type_info()
    let rel_pairs: Vec<(&str, &str)> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Relationship))
        .filter_map(|f| {
            let wire = f.wire_name.as_deref()?;
            let target = f.rel_target_type.as_deref()?;
            Some((wire, target))
        })
        .collect();

    let rel_name_tokens: Vec<&str> = rel_pairs.iter().map(|(n, _)| *n).collect();
    let rel_target_tokens: Vec<&str> = rel_pairs.iter().map(|(_, t)| *t).collect();

    quote! {
        impl ::jsonapi_core::model::ResourceObject for #name {
            fn resource_type(&self) -> &str {
                #type_name
            }

            fn resource_id(&self) -> ::core::option::Option<&str> {
                #id_expr
            }

            fn resource_lid(&self) -> ::core::option::Option<&str> {
                #lid_expr
            }

            fn field_names() -> &'static [&'static str] {
                &[#(#field_name_strs),*]
            }

            fn type_info() -> ::jsonapi_core::TypeInfo where Self: Sized {
                ::jsonapi_core::TypeInfo::new(
                    #type_name,
                    &[#(#field_name_strs),*],
                    &[#((#rel_name_tokens, #rel_target_tokens)),*],
                )
            }
        }
    }
}

fn gen_serialize(
    name: &syn::Ident,
    struct_attrs: &StructAttrs,
    fields: &[ParsedField],
) -> TokenStream {
    let type_name = &struct_attrs.type_name;

    // id field
    let id_field = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Id))
        .unwrap();
    let id_ident = &id_field.ident;
    let id_entry = if id_field.is_option {
        quote! {
            if let ::core::option::Option::Some(ref __id) = self.#id_ident {
                __map.serialize_entry("id", __id)?;
            }
        }
    } else {
        quote! {
            __map.serialize_entry("id", &self.#id_ident)?;
        }
    };

    // lid field
    let lid_entry = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Lid))
        .map(|f| {
            let lid_ident = &f.ident;
            quote! {
                if let ::core::option::Option::Some(ref __lid) = self.#lid_ident {
                    __map.serialize_entry("lid", __lid)?;
                }
            }
        })
        .unwrap_or_default();

    // Attribute fields
    let attr_fields: Vec<&ParsedField> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Attribute))
        .collect();

    let attrs_entry = if attr_fields.is_empty() {
        quote! {}
    } else {
        let attr_inserts: Vec<TokenStream> = attr_fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                let wire = f.wire_name.as_ref().unwrap();
                if f.is_option {
                    quote! {
                        if let ::core::option::Option::Some(ref __val) = self.#ident {
                            __attrs.insert(
                                #wire.to_string(),
                                ::serde_json::to_value(__val).map_err(::serde::ser::Error::custom)?,
                            );
                        }
                    }
                } else {
                    quote! {
                        __attrs.insert(
                            #wire.to_string(),
                            ::serde_json::to_value(&self.#ident).map_err(::serde::ser::Error::custom)?,
                        );
                    }
                }
            })
            .collect();

        quote! {
            {
                let mut __attrs = ::serde_json::Map::new();
                #(#attr_inserts)*
                if !__attrs.is_empty() {
                    __map.serialize_entry("attributes", &__attrs)?;
                }
            }
        }
    };

    // Relationship fields
    let rel_fields: Vec<&ParsedField> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Relationship))
        .collect();

    let rels_entry = if rel_fields.is_empty() {
        quote! {}
    } else {
        let rel_inserts: Vec<TokenStream> = rel_fields
            .iter()
            .map(|f| {
                let ident = &f.ident;
                let wire = f.wire_name.as_ref().unwrap();
                if f.is_option {
                    quote! {
                        if let ::core::option::Option::Some(ref __val) = self.#ident {
                            __rels.insert(
                                #wire.to_string(),
                                ::serde_json::to_value(__val).map_err(::serde::ser::Error::custom)?,
                            );
                        }
                    }
                } else {
                    quote! {
                        __rels.insert(
                            #wire.to_string(),
                            ::serde_json::to_value(&self.#ident).map_err(::serde::ser::Error::custom)?,
                        );
                    }
                }
            })
            .collect();

        quote! {
            {
                let mut __rels = ::serde_json::Map::new();
                #(#rel_inserts)*
                if !__rels.is_empty() {
                    __map.serialize_entry("relationships", &__rels)?;
                }
            }
        }
    };

    // links field
    let links_entry = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Links))
        .map(|f| {
            let ident = &f.ident;
            quote! {
                if let ::core::option::Option::Some(ref __links) = self.#ident {
                    __map.serialize_entry("links", __links)?;
                }
            }
        })
        .unwrap_or_default();

    // meta field
    let meta_entry = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Meta))
        .map(|f| {
            let ident = &f.ident;
            quote! {
                if let ::core::option::Option::Some(ref __meta) = self.#ident {
                    __map.serialize_entry("meta", __meta)?;
                }
            }
        })
        .unwrap_or_default();

    quote! {
        impl ::serde::Serialize for #name {
            fn serialize<__S: ::serde::Serializer>(
                &self,
                serializer: __S,
            ) -> ::core::result::Result<__S::Ok, __S::Error> {
                use ::serde::ser::SerializeMap;

                let mut __map = serializer.serialize_map(::core::option::Option::None)?;

                __map.serialize_entry("type", #type_name)?;
                #id_entry
                #lid_entry
                #attrs_entry
                #rels_entry
                #links_entry
                #meta_entry

                __map.end()
            }
        }
    }
}

fn gen_deserialize(
    name: &syn::Ident,
    struct_attrs: &StructAttrs,
    fields: &[ParsedField],
) -> TokenStream {
    let type_name = &struct_attrs.type_name;

    // id field
    let id_field = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Id))
        .unwrap();
    let id_ident = &id_field.ident;
    let id_extract = if id_field.is_option {
        quote! {
            let #id_ident: ::core::option::Option<String> = __obj
                .get("id")
                .and_then(|v| v.as_str())
                .map(::std::string::String::from);
        }
    } else {
        quote! {
            let #id_ident: String = __obj
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ::serde::de::Error::custom("resource must have an `id` string"))?
                .to_string();
        }
    };

    // lid field
    let lid_extract = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Lid))
        .map(|f| {
            let ident = &f.ident;
            quote! {
                let #ident: ::core::option::Option<String> = __obj
                    .get("lid")
                    .and_then(|v| v.as_str())
                    .map(::std::string::String::from);
            }
        });

    // Attribute fields
    let attr_extracts: Vec<TokenStream> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Attribute))
        .map(|f| gen_field_extract(f, "__attrs"))
        .collect();

    // Relationship fields
    let rel_extracts: Vec<TokenStream> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Relationship))
        .map(|f| gen_field_extract(f, "__rels"))
        .collect();

    // links field
    let links_extract = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Links))
        .map(|f| {
            let ident = &f.ident;
            quote! {
                let #ident = __obj
                    .get("links")
                    .map(|v| ::serde_json::from_value(v.clone()).map_err(::serde::de::Error::custom))
                    .transpose()?;
            }
        });

    // meta field
    let meta_extract = fields
        .iter()
        .find(|f| matches!(f.kind, FieldKind::Meta))
        .map(|f| {
            let ident = &f.ident;
            quote! {
                let #ident = __obj
                    .get("meta")
                    .map(|v| ::serde_json::from_value(v.clone()).map_err(::serde::de::Error::custom))
                    .transpose()?;
            }
        });

    // skip fields — initialize with Default
    let skip_inits: Vec<TokenStream> = fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Skip))
        .map(|f| {
            let ident = &f.ident;
            quote! { let #ident = ::core::default::Default::default(); }
        })
        .collect();

    // Struct constructor
    let field_idents: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();

    quote! {
        impl<'de> ::serde::Deserialize<'de> for #name {
            fn deserialize<__D: ::serde::Deserializer<'de>>(
                deserializer: __D,
            ) -> ::core::result::Result<Self, __D::Error> {
                let __value = <::serde_json::Value as ::serde::Deserialize>::deserialize(deserializer)?;
                let __obj = __value
                    .as_object()
                    .ok_or_else(|| ::serde::de::Error::custom("resource must be a JSON object"))?;

                // Validate type
                let __type_str = __obj
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ::serde::de::Error::custom("resource must have a `type` string"))?;
                if __type_str != #type_name {
                    return ::core::result::Result::Err(::serde::de::Error::custom(
                        ::std::format!("expected type \"{}\", got \"{}\"", #type_name, __type_str),
                    ));
                }

                #id_extract
                #lid_extract

                let __attrs = __obj.get("attributes").and_then(|v| v.as_object());
                #(#attr_extracts)*

                let __rels = __obj.get("relationships").and_then(|v| v.as_object());
                #(#rel_extracts)*

                #links_extract
                #meta_extract
                #(#skip_inits)*

                ::core::result::Result::Ok(#name {
                    #(#field_idents),*
                })
            }
        }
    }
}

/// Generate the extraction code for a single attribute or relationship field.
fn gen_field_extract(field: &ParsedField, source_var: &str) -> TokenStream {
    let ident = &field.ident;
    let ty = &field.ty;
    let source = format_ident!("{}", source_var);

    // Build the lookup chain
    let lookup = if field.aliases.is_empty() {
        // Single lookup (no aliases, or renamed field)
        let wire = field.wire_name.as_ref().unwrap();
        quote! { #source.and_then(|__s| __s.get(#wire)) }
    } else {
        // Fuzzy alias chain: try output case first, then alternatives
        let first = &field.aliases[0];
        let mut chain = quote! { #source.and_then(|__s| __s.get(#first)) };
        for alias in &field.aliases[1..] {
            chain = quote! {
                #chain.or_else(|| #source.and_then(|__s| __s.get(#alias)))
            };
        }
        chain
    };

    if field.is_option {
        quote! {
            let #ident: #ty = #lookup
                .map(|v| ::serde_json::from_value(v.clone()).map_err(::serde::de::Error::custom))
                .transpose()?;
        }
    } else if field.is_vec {
        quote! {
            let #ident: #ty = match #lookup {
                ::core::option::Option::Some(v) => {
                    ::serde_json::from_value(v.clone()).map_err(::serde::de::Error::custom)?
                }
                ::core::option::Option::None => ::core::default::Default::default(),
            };
        }
    } else {
        let wire = field.wire_name.as_ref().unwrap();
        quote! {
            let __raw = #lookup
                .ok_or_else(|| ::serde::de::Error::missing_field(#wire))?
                .clone();
            let #ident: #ty = ::serde_json::from_value(__raw)
                .map_err(::serde::de::Error::custom)?;
        }
    }
}
