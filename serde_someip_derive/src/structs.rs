use super::attribute::*;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{Error, Result};
use syn::spanned::Spanned;
use syn::{
    Attribute, DataStruct, Expr, Fields, FieldsNamed, FieldsUnnamed, GenericArgument, Ident,
    LitInt, LitStr, PathArguments, Type,
};

use std::collections::HashSet;

pub(crate) fn derive(attrs: &[Attribute], data: DataStruct, ident: &Ident) -> TokenStream {
    if data.fields.is_empty() {
        return quote_spanned! {data.struct_token.span=>
            compile_error!("Empty structs are not supported by someip")
        };
    }

    let res = match data.fields {
        Fields::Named(fields) => derive_normal_struct(attrs, fields, ident),
        Fields::Unnamed(fields) => derive_newtype_struct(attrs, fields, data.struct_token.span),
        Fields::Unit => unreachable!(),
    };
    if let Err(e) = res {
        e.to_compile_error()
    } else {
        res.unwrap()
    }
}

fn derive_normal_struct(
    attrs: &[Attribute],
    fields: FieldsNamed,
    ident: &Ident,
) -> Result<TokenStream> {
    let (lfsize, is_message_wrapper) = if let Some(attr) = get_optional_someip_attr(attrs)? {
        attr.check(
            &[],
            &[
                ("message_wrapper", AttributeValueType::Bool),
                ("length_field_size", AttributeValueType::Int),
            ],
            &[],
        )?;
        let lfsize = derive_length_field_size(&attr)?;
        let is_message_wrapper = if let Some(v) = attr.get_optional("message_wrapper") {
            v.as_ref().unwrap_bool().value
        } else {
            false
        };
        (lfsize, is_message_wrapper)
    } else {
        (quote! {None}, false)
    };

    let mut encountered_ids = 0;
    let mut seen_ids: HashSet<u16> = HashSet::default();
    let mut derived_fields = Vec::with_capacity(fields.named.len());
    for field in fields.named {
        let attrs = get_optional_someip_attr(&field.attrs)?;
        let ident = field.ident.unwrap();

        let id = if let Some(attrs) = &attrs {
            attrs.check(&[], &[("id", AttributeValueType::Int)], &["*"])?;
            if let Some(id) = attrs.get_optional("id") {
                let id = id.as_ref().unwrap_int();
                encountered_ids += 1;
                let parsed_id = id.base10_parse::<u16>()?;
                if !seen_ids.insert(parsed_id) {
                    quote_spanned! {id.span()=>
                        compile_error!("Ids must be unique within the struct")
                    }
                } else if parsed_id > 0xFFF {
                    quote_spanned! {id.span()=>
                        compile_error!("The id must be between 0 and 0xFFF inclusive")
                    }
                } else {
                    quote! {Some(#id)}
                }
            } else {
                quote! {None}
            }
        } else {
            quote! {None}
        };

        let ty = derive_type(
            attrs.as_ref(),
            &field.ty,
            ident.span(),
            true,
            encountered_ids > 0,
        )
        .unwrap_or_else(|e| e.to_compile_error());

        let ident = LitStr::new(&ident.to_string(), ident.span());
        derived_fields.push(quote! {
            serde_someip::types::SomeIpField{
                name: #ident,
                id: #id,
                field_type: &#ty,
            }
        });
    }

    if encountered_ids != 0 && encountered_ids != derived_fields.len() {
        return Err(Error::new(
            fields.brace_token.span,
            "Either all fields or none must have an id",
        ));
    }

    let is_tlv = encountered_ids > 0;
    let name = LitStr::new(&ident.to_string(), ident.span());

    Ok(quote! {
        serde_someip::types::SomeIpType::Struct(serde_someip::types::SomeIpStruct {
            name: #name,
            fields: &[#(#derived_fields),*],
            uses_tlv_serialization: #is_tlv,
            is_message_wrapper: #is_message_wrapper,
            length_field_size: #lfsize,
        })
    })
}

fn derive_newtype_struct(
    attrs: &[Attribute],
    fields: FieldsUnnamed,
    span: Span,
) -> Result<TokenStream> {
    if fields.unnamed.len() != 1 {
        return Err(Error::new(
            fields.paren_token.span,
            "Only newtype structs (tuple structs with exactly one paramter) are supported",
        ));
    }
    let field = fields.unnamed.first().unwrap();
    derive_type(
        get_optional_someip_attr(attrs)?.as_ref(),
        &field.ty,
        span,
        true,
        false,
    )
}

fn derive_type(
    attrs: Option<&SomeIpAttribute>,
    ty: &Type,
    span: Span,
    is_outer: bool,
    is_in_tlv_struct: bool,
) -> Result<TokenStream> {
    let ty = handle_treat_as_attribute(attrs, ty)?;
    let ty = resolve_type(&ty);

    if let Some(elem) = is_option(ty) {
        if !is_in_tlv_struct {
            return Err(Error::new(
                ty.span(),
                "Cannot use Option outside of TLV struct",
            ));
        }
        derive_type(attrs, elem, span, is_outer, is_in_tlv_struct)
    } else if is_string(ty) {
        derive_string_type(attrs, span, is_outer)
    } else if let Some(elem) = is_sequence(ty) {
        derive_sequence_type(attrs, elem, span, is_outer)
    } else if let Some((elem, len)) = is_array(ty) {
        derive_array_type(attrs, elem, len, span)
    } else {
        Ok(quote! {#ty::SOMEIP_TYPE})
    }
}

fn derive_string_type(
    attr: Option<&SomeIpAttribute>,
    span: Span,
    is_outer: bool,
) -> Result<TokenStream> {
    let attr = attr.ok_or_else(|| {
        let message = if is_outer {
            "A someip attribute is required for string types"
        } else {
            "A elements attribute is required for element string types"
        };
        Error::new(span, message)
    })?;
    attr.check(
        &[("max_size", AttributeValueType::Int)],
        &[
            ("min_size", AttributeValueType::Int),
            ("length_field_size", AttributeValueType::Int),
        ],
        &["treat_as", "id"],
    )?;
    let lfsize = derive_length_field_size(attr)?;
    let min_size = get_min_sizes(attr, "min_size");
    let max_size = attr.get("max_size").as_ref().unwrap_int();
    Ok(
        quote! {serde_someip::types::SomeIpType::String(serde_someip::types::SomeIpString {
            min_size: #min_size,
            max_size: #max_size,
            length_field_size: #lfsize
        })},
    )
}

fn derive_sequence_type(
    attr: Option<&SomeIpAttribute>,
    elem: &Type,
    span: Span,
    is_outer: bool,
) -> Result<TokenStream> {
    let attr = attr.ok_or_else(|| {
        let message = if is_outer {
            "A someip attribute is required for sequence types (slices, vecs, etc.)"
        } else {
            "A elements attribute is required for element sequence types (slices, vecs, etc.)"
        };
        Error::new(span, message)
    })?;

    attr.check(
        &[("max_elements", AttributeValueType::Int)],
        &[
            ("min_elements", AttributeValueType::Int),
            ("length_field_size", AttributeValueType::Int),
            ("elements", AttributeValueType::Inner),
        ],
        &["treat_as", "id"],
    )?;

    let min_elements = get_min_sizes(attr, "min_elements");
    let max_elements = attr.get("max_elements").as_ref().unwrap_int();

    let lfsize = derive_length_field_size(attr)?;

    let inner_attr = attr
        .get_optional("elements")
        .map(|a| a.as_ref().unwrap_inner());
    let element_type = derive_type(inner_attr, elem, attr.span, false, false)?;

    Ok(
        quote! {serde_someip::types::SomeIpType::Sequence(serde_someip::types::SomeIpSequence {
            min_elements: #min_elements,
            max_elements: #max_elements,
            element_type: &#element_type,
            length_field_size: #lfsize,
        })},
    )
}

fn derive_array_type(
    attr: Option<&SomeIpAttribute>,
    elem: &Type,
    len: &Expr,
    span: Span,
) -> Result<TokenStream> {
    let (lfsize, inner_attr, span) = if let Some(attr) = attr {
        attr.check(
            &[],
            &[
                ("length_field_size", AttributeValueType::Int),
                ("elements", AttributeValueType::Inner),
            ],
            &["treat_as", "id"],
        )?;
        let lfsize = derive_length_field_size(attr)?;
        (
            lfsize,
            attr.get_optional("elements")
                .map(|a| a.as_ref().unwrap_inner()),
            attr.span,
        )
    } else {
        (quote! {None}, None, span)
    };

    let element_type = derive_type(inner_attr, elem, span, false, false)?;

    Ok(
        quote! {serde_someip::types::SomeIpType::Sequence(serde_someip::types::SomeIpSequence {
            min_elements: #len,
            max_elements: #len,
            element_type: &#element_type,
            length_field_size: #lfsize,
        })},
    )
}

#[inline]
fn get_min_sizes(attr: &SomeIpAttribute, key: &str) -> LitInt {
    if let Some(attr) = attr.get_optional(key) {
        attr.as_ref().unwrap_int().clone()
    } else {
        LitInt::new("0", attr.span)
    }
}

#[inline]
fn derive_length_field_size(attr: &SomeIpAttribute) -> Result<TokenStream> {
    if let Some(attr) = attr.get_optional("length_field_size") {
        attr.to_length_field_size()
    } else {
        Ok(quote! {None})
    }
}

#[inline]
fn handle_treat_as_attribute(attr: Option<&SomeIpAttribute>, ty: &Type) -> Result<Type> {
    if let Some(attr) = attr {
        attr.check(&[], &[("treat_as", AttributeValueType::Type)], &["*"])?;
        if let Some(kv) = attr.get_optional("treat_as") {
            return Ok(kv.as_ref().unwrap_type().clone());
        }
    }
    Ok(ty.clone())
}

fn resolve_type(ty: &Type) -> &Type {
    match ty {
        Type::Ptr(ptr) => resolve_type(&ptr.elem),
        Type::Reference(rf) => resolve_type(&rf.elem),
        _ => ty,
    }
}

#[inline]
fn is_string(ty: &Type) -> bool {
    match ty {
        Type::Path(p) => p.path.is_ident("str") || p.path.is_ident("String"),
        _ => false,
    }
}

#[inline]
fn is_sequence(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Slice(s) => return Some(&s.elem),
        Type::Path(p) => {
            if let Some(v) = p.path.segments.iter().next() {
                if v.ident == "Vec" {
                    if let PathArguments::AngleBracketed(args) = &v.arguments {
                        if let Some(GenericArgument::Type(ty)) = args.args.iter().next() {
                            return Some(ty);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    None
}

#[inline]
fn is_option(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(v) = p.path.segments.iter().next() {
            if v.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &v.arguments {
                    if let Some(GenericArgument::Type(ty)) = args.args.iter().next() {
                        return Some(ty);
                    }
                }
            }
        }
    }
    None
}

#[inline]
fn is_array(ty: &Type) -> Option<(&Type, &Expr)> {
    match ty {
        Type::Array(a) => Some((&a.elem, &a.len)),
        _ => None,
    }
}
