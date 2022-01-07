use syn::parse::{Error, Parse, ParseStream, Result};
use syn::spanned::Spanned;
use syn::{parenthesized, parse2, token::Paren, Attribute, Ident, LitBool, LitInt, Token, Type};

use quote::quote;

use proc_macro2::{Span, TokenStream};

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributeValueType {
    Bool,
    Int,
    Type,
    Inner,
}

impl Display for AttributeValueType {
    fn fmt(&self, fmt: &mut Formatter) -> std::fmt::Result {
        match self {
            AttributeValueType::Bool => fmt.write_str("bool"),
            AttributeValueType::Int => fmt.write_str("integer"),
            AttributeValueType::Type => fmt.write_str("type"),
            AttributeValueType::Inner => fmt.write_str("(values, ...)"),
        }
    }
}

pub(crate) enum AttributeValue {
    Bool(LitBool),
    Int(LitInt),
    Type(Type),
    Inner(SomeIpAttribute),
}

impl AttributeValue {
    #[inline]
    fn matches(&self, expected: &AttributeValueType) -> bool {
        match self {
            AttributeValue::Bool(_) => expected == &AttributeValueType::Bool,
            AttributeValue::Int(_) => expected == &AttributeValueType::Int,
            AttributeValue::Type(_) => expected == &AttributeValueType::Type,
            AttributeValue::Inner(_) => expected == &AttributeValueType::Inner,
        }
    }

    #[inline]
    fn display_type(&self) -> impl Display {
        match self {
            AttributeValue::Bool(_) => AttributeValueType::Bool,
            AttributeValue::Int(_) => AttributeValueType::Int,
            AttributeValue::Type(_) => AttributeValueType::Type,
            AttributeValue::Inner(_) => AttributeValueType::Inner,
        }
    }

    #[inline]
    pub(crate) fn span(&self) -> Span {
        match self {
            AttributeValue::Bool(v) => v.span(),
            AttributeValue::Int(v) => v.span(),
            AttributeValue::Type(v) => v.span(),
            AttributeValue::Inner(v) => v.span,
        }
    }

    #[inline]
    pub(crate) fn unwrap_bool(&self) -> &LitBool {
        match self {
            AttributeValue::Bool(v) => v,
            _ => panic!(),
        }
    }

    #[inline]
    pub(crate) fn unwrap_int(&self) -> &LitInt {
        match self {
            AttributeValue::Int(v) => v,
            _ => panic!(),
        }
    }

    #[inline]
    pub(crate) fn unwrap_type(&self) -> &Type {
        match self {
            AttributeValue::Type(v) => v,
            _ => panic!(),
        }
    }

    #[inline]
    pub(crate) fn unwrap_inner(&self) -> &SomeIpAttribute {
        match self {
            AttributeValue::Inner(v) => v,
            _ => panic!(),
        }
    }
}

pub(crate) struct AttributeKeyValue {
    ident: Ident,
    pub(crate) value: AttributeValue,
}

impl AttributeKeyValue {
    #[inline]
    pub(crate) fn to_length_field_size(&self) -> Result<TokenStream> {
        if let AttributeValue::Int(v) = &self.value {
            match v.base10_parse::<u8>()? {
                1 => {
                    return Ok(quote! {Some(serde_someip::length_fields::LengthFieldSize::OneByte)})
                }
                2 => {
                    return Ok(
                        quote! {Some(serde_someip::length_fields::LengthFieldSize::TwoBytes)},
                    )
                }
                4 => {
                    return Ok(
                        quote! {Some(serde_someip::length_fields::LengthFieldSize::FourBytes)},
                    )
                }
                _ => {}
            }
        }

        Err(Error::new(
            self.value.span(),
            format!("Attribute {} must have a value of 1, 2, or 4", self.ident),
        ))
    }
}

impl AsRef<AttributeValue> for AttributeKeyValue {
    fn as_ref(&self) -> &AttributeValue {
        &self.value
    }
}

impl AsMut<AttributeValue> for AttributeKeyValue {
    fn as_mut(&mut self) -> &mut AttributeValue {
        &mut self.value
    }
}

pub(crate) struct SomeIpAttribute {
    pub(crate) span: Span,
    data: HashMap<String, AttributeKeyValue>,
}

impl SomeIpAttribute {
    #[inline]
    pub(crate) fn get_optional(&self, key: &str) -> Option<&AttributeKeyValue> {
        self.data.get(key)
    }

    #[inline]
    pub(crate) fn get(&self, key: &str) -> &AttributeKeyValue {
        self.get_optional(key).unwrap()
    }

    pub(crate) fn check(
        &self,
        required: &[(&str, AttributeValueType)],
        optional: &[(&str, AttributeValueType)],
        ignored: &[&str],
    ) -> Result<()> {
        let mut encountered_keys: HashSet<&str> = HashSet::default();
        for (key, expected_value) in required {
            if let Some(kv) = self.get_optional(key) {
                encountered_keys.insert(*key);
                if !kv.value.matches(expected_value) {
                    let message = format!(
                        "Attribute {} is of wrong type, expected {}, actual {}",
                        key,
                        expected_value,
                        kv.value.display_type()
                    );
                    return Err(Error::new(kv.value.span(), message));
                }
            } else {
                let message = format!("Required attribute {} not found", key);
                return Err(Error::new(self.span, message));
            }
        }

        for (key, expected_value) in optional {
            if let Some(kv) = self.get_optional(key) {
                encountered_keys.insert(*key);
                if !kv.value.matches(expected_value) {
                    let message = format!(
                        "Attribute {} is of wrong type, expected {}, actual {}",
                        key,
                        expected_value,
                        kv.value.display_type()
                    );
                    return Err(Error::new(kv.value.span(), message));
                }
            }
        }

        if ignored.contains(&"*") {
            return Ok(());
        }

        for key in ignored {
            if self.data.contains_key(*key) {
                encountered_keys.insert(*key);
            }
        }

        if encountered_keys.len() != self.data.len() {
            for (key, value) in &self.data {
                if !encountered_keys.contains(key as &str) {
                    let message = format!("Unknown attribute: {}", key);
                    return Err(Error::new(value.ident.span(), message));
                }
            }
        }
        Ok(())
    }
}

impl Parse for AttributeKeyValue {
    fn parse(input: ParseStream) -> Result<AttributeKeyValue> {
        let ident: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let lookahead = input.lookahead1();
        let value = if lookahead.peek(LitBool) {
            AttributeValue::Bool(input.parse()?)
        } else if lookahead.peek(LitInt) {
            AttributeValue::Int(input.parse()?)
        } else if lookahead.peek(Paren) {
            AttributeValue::Inner(input.parse()?)
        } else {
            AttributeValue::Type(input.parse()?)
        };
        Ok(AttributeKeyValue { ident, value })
    }
}

impl Parse for SomeIpAttribute {
    fn parse(input: ParseStream) -> Result<SomeIpAttribute> {
        let content;
        let span = parenthesized!(content in input).span;
        let values = content.parse_terminated::<_, Token![,]>(AttributeKeyValue::parse)?;
        let mut data = HashMap::with_capacity(values.len());
        for value in values {
            match data.entry(value.ident.to_string()) {
                Entry::Occupied(_) => {
                    return Err(Error::new(value.ident.span(), "Duplicate attribute"))
                }
                Entry::Vacant(v) => v.insert(value),
            };
        }
        Ok(SomeIpAttribute { span, data })
    }
}

#[inline]
pub(crate) fn get_optional_someip_attr(
    attributes: &[Attribute],
) -> Result<Option<SomeIpAttribute>> {
    let attr = attributes.iter().find(|a| a.path.is_ident("someip"));
    if let Some(attr) = attr {
        Ok(Some(parse2(attr.tokens.clone())?))
    } else {
        Ok(None)
    }
}

#[inline]
pub(crate) fn get_someip_attr(attributes: &[Attribute], span: Span) -> Result<SomeIpAttribute> {
    get_optional_someip_attr(attributes)?
        .ok_or_else(|| Error::new(span, "Missing someip attribute"))
}
