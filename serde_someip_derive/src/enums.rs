use super::attribute::*;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Error, Result};
use syn::spanned::Spanned;
use syn::{Attribute, DataEnum, Fields, Ident, LitInt, LitStr, Type, Variant};

use std::collections::HashSet;

#[derive(Clone, Copy)]
enum RawType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

impl RawType {
    #[inline]
    fn into_tokens(self) -> TokenStream {
        match self {
            RawType::U8 => quote! {serde_someip::types::SomeIpEnumValue::U8},
            RawType::U16 => quote! {serde_someip::types::SomeIpEnumValue::U16},
            RawType::U32 => quote! {serde_someip::types::SomeIpEnumValue::U32},
            RawType::U64 => quote! {serde_someip::types::SomeIpEnumValue::U64},
            RawType::I8 => quote! {serde_someip::types::SomeIpEnumValue::I8},
            RawType::I16 => quote! {serde_someip::types::SomeIpEnumValue::I16},
            RawType::I32 => quote! {serde_someip::types::SomeIpEnumValue::I32},
            RawType::I64 => quote! {serde_someip::types::SomeIpEnumValue::I64},
        }
    }

    #[inline]
    fn parse(&self, value: &LitInt) -> Result<u64> {
        Ok(match self {
            RawType::U8 => value.base10_parse::<u8>()? as u64,
            RawType::U16 => value.base10_parse::<u16>()? as u64,
            RawType::U32 => value.base10_parse::<u32>()? as u64,
            RawType::U64 => value.base10_parse::<u64>()?,
            RawType::I8 => value.base10_parse::<i8>()? as u8 as u64,
            RawType::I16 => value.base10_parse::<i16>()? as u16 as u64,
            RawType::I32 => value.base10_parse::<i32>()? as u32 as u64,
            RawType::I64 => value.base10_parse::<i64>()? as u64,
        })
    }
}

pub(crate) fn derive(attrs: &[Attribute], data: DataEnum, ident: &Ident) -> TokenStream {
    let raw_type_result = parse_raw_type(attrs, data.enum_token.span);
    if let Err(e) = raw_type_result {
        return e.to_compile_error();
    }
    let (raw_type, enum_value_type) = raw_type_result.unwrap();
    let mut seen_values = HashSet::default();
    let values = data.variants.iter().map(|v| {
        parse_value(v, enum_value_type, &mut seen_values).unwrap_or_else(|e| e.to_compile_error())
    });
    let name = LitStr::new(&ident.to_string(), ident.span());
    quote! {
            serde_someip::types::SomeIpType::Enum(serde_someip::types::SomeIpEnum {
                name: #name,
                raw_type: #raw_type,
                values: &[#(#values),*],
            })
    }
}

fn parse_raw_type(attrs: &[Attribute], span: Span) -> Result<(TokenStream, RawType)> {
    let attr = get_someip_attr(attrs, span)?;
    attr.check(&[("raw_type", AttributeValueType::Type)], &[], &[])?;

    let raw_type = attr.get("raw_type").as_ref().unwrap_type();
    let raw_type = if let Type::Path(p) = raw_type {
        &p.path
    } else {
        return Err(Error::new(
            raw_type.span(),
            "Unsupported raw_type: Only u8, u16, u32, u64, i8, i16, i32 or i64 are supported",
        ));
    };

    if raw_type.is_ident("u8") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::U8},
            RawType::U8,
        ))
    } else if raw_type.is_ident("u16") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::U16},
            RawType::U16,
        ))
    } else if raw_type.is_ident("u32") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::U32},
            RawType::U32,
        ))
    } else if raw_type.is_ident("u64") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::U64},
            RawType::U64,
        ))
    } else if raw_type.is_ident("i8") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::I8},
            RawType::I8,
        ))
    } else if raw_type.is_ident("i16") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::I16},
            RawType::I16,
        ))
    } else if raw_type.is_ident("i32") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::I32},
            RawType::I32,
        ))
    } else if raw_type.is_ident("i64") {
        Ok((
            quote! {serde_someip::types::SomeIpPrimitive::I64},
            RawType::I64,
        ))
    } else {
        Err(Error::new(
            raw_type.span(),
            "Unsupported raw_type: Only u8, u16, u32, u64, i8, i16, i32 or i64 are supported",
        ))
    }
}

fn parse_value(
    variant: &Variant,
    raw_type: RawType,
    seen_values: &mut HashSet<u64>,
) -> Result<TokenStream> {
    match variant.fields {
        Fields::Unit => {}
        _ => {
            return Err(Error::new(
                variant.ident.span(),
                "SomeIp only supports unit enum variants",
            ))
        }
    }

    let attr = get_someip_attr(&variant.attrs, variant.ident.span())?;
    attr.check(&[("value", AttributeValueType::Int)], &[], &[])?;

    let ident = LitStr::new(&variant.ident.to_string(), variant.ident.span());
    let value = attr.get("value").as_ref().unwrap_int();
    let parsed_value = raw_type.parse(value)?;
    if !seen_values.insert(parsed_value) {
        return Err(Error::new(value.span(), "Duplicate value"));
    }
    let enum_value_type = raw_type.into_tokens();
    Ok(quote! {(#ident, #enum_value_type(#value))})
}
