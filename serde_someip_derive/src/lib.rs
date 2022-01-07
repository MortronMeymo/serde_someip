//! Provides the `#[derive(SomeIp)]` functionality for the `serde_someip` crate.
#![deny(missing_docs)]

use quote::{quote, quote_spanned};
use syn::{parse_macro_input, Data, DeriveInput};

pub(crate) mod attribute;
mod enums;
mod structs;

/// Use via the `serde_someip` crate with feature `derive`.
#[proc_macro_derive(SomeIp, attributes(someip))]
pub fn derive_someip(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let someip_type = match input.data {
        Data::Struct(s) => structs::derive(&input.attrs, s, &ident),
        Data::Enum(e) => enums::derive(&input.attrs, e, &ident),
        Data::Union(u) => {
            return quote_spanned! {u.union_token.span=>
                compile_error!("Unions are not supported by someip");
            }
            .into();
        }
    };

    quote! {
        impl #impl_generics serde_someip::types::SomeIp for #ident #ty_generics #where_clause {
            const SOMEIP_TYPE: serde_someip::types::SomeIpType = #someip_type;
        }
    }
    .into()
}
