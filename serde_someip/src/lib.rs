//! Implements the [`SOME/IP`] serialization format as defined by autosar for the serde framework.
//!
//! This data format is commonly used in automotive applications that communicate over ethernet.
//!
//! This crate does not aim to provide a full someip stack instead it only deals with the serialization of
//! Data Structures (chapter 4.1.4 of the linked spec) but fully handles that part.
//!
//! [`SOME/IP`]: https://www.autosar.org/fileadmin/user_upload/standards/foundation/19-11/AUTOSAR_PRS_SOMEIPProtocol.pdf
#![deny(missing_docs)]

pub mod de;
pub mod error;
pub mod ser;

#[cfg(feature = "bytes")]
pub use de::from_bytes;
pub use de::{from_reader, from_slice};
pub use error::{Error, Result};
#[cfg(feature = "bytes")]
pub use ser::{append_to_bytes, to_bytes};
pub use ser::{append_to_vec, to_vec};

pub mod length_fields;
pub mod options;
pub mod types;

pub(crate) mod wire_type;

pub use options::SomeIpOptions;
pub use types::SomeIp;

#[cfg(feature = "derive")]
extern crate serde_someip_derive;
#[cfg(feature = "derive")]
/// Provides #[derive(SomeIp)] requires the `derive` feature.
///
/// *Only available with the `derive` feature.*
///
/// # Length Field Sizes
/// For multiple types you can specify a `length_field_size` this is always optional.
/// If you provide it it must be either `1`, `2` or `4` since those are the only
/// length field sizes supported by someip. If you do not provide it and the type
/// needs a length field size to be serialized it will be taken from the [SomeIpOptions]
/// if the options also do not provide one the de/serialization will panic.
///
/// # Eums
/// For enums you must provide a `raw_type` to indicate how to serialize the enum
/// and every variant must be assigned a `value` that is valid for that raw_type.
/// SomeIp only supports unit variants, so tuple and struct variants will cause a compile error.
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(raw_type = i16)]
/// enum Foo {
///     #[someip(value = -10)]
///     Bar
/// }
/// ```
/// ```compile_fail
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(raw_type = i16)]
/// enum Foo {
///     #[someip(value = -10)]
///     Bar(u8)
/// }
/// ```
///
/// # Strings
/// For strings you must provide the `max_size` and can optionally provide a `min_size` (defaults to 0 if not present)
///  and a `length_field_size`. The size is in bytes after encoding the string with the string encoding specified by [SomeIpOptions].
/// Since this can be different for every string you must provide it for every string. If you want to just
/// serialize a string outside of a struct you must wrap it in a newtype struct.
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(max_size = 42)] //this is the mimium required attrributes
/// struct AString(String);
///
/// #[derive(SomeIp)]
/// #[someip(min_size = 10, max_size = 42, length_field_size = 2)] //this is the maximum possible attributes
/// struct AnotherString<'a>(&'a str);
/// ```
///
/// # Sequences
/// For Sequences you must provide the `max_elements` inside the sequence and can provide a `min_elements` (defaults to 0 if not present)
/// and a `length_field_size`:
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(max_elements = 42)] //this is the mimium required attrributes
/// struct AVec(Vec<u32>);
///
/// #[derive(SomeIp)]
/// #[someip(min_elements = 10, max_elements = 42, length_field_size = 2)]
/// struct ASlice<'a>(&'a [u32]);
/// ```
/// If the element type of your sequence requires additional information like for example a [String] does you must provide this via the
/// `elements` attribute:
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(max_elements = 42, elements = (max_size = 10))] //this is the mimium required attrributes
/// struct AVec(Vec<String>);
/// ```
/// Of course you can recurse `elements` for multidimensional arrays:
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// #[someip(max_elements = 42, elements = (max_elements = 3, elements = (max_size = 10)))] //this is the mimium required attrributes
/// struct AVec(Vec<Vec<String>>);
/// ```
///
/// # Structs
/// Structs can have `message_wrapper`, `length_field_size`, `arrays_length_field_size`, `structs_length_field_size` and a `strings_length_field_size` attribute
/// though none are required. The three `xs_length_field_size` attributes correspond to similarily named values in the someip transformation properties of autosar.
/// When present these attribtues will set the length_field_sizes for all matching types used in this struct.
/// The `message_wrapper` attribute indicates that this struct is a wrapper for a someip message
/// (the parameters/return values of a function call or the data of an event) this must be considered during de/serialization
/// since such structs must not beginn with a length field.
/// The attributes for strings and sequences are put on the appropiate fields:
/// ```
/// # use serde_someip::SomeIp;
/// //minimal example
/// #[derive(SomeIp)]
/// struct AStruct {
///     #[someip(max_size = 42)]
///     foo: String,
/// };
///
/// //example with all possible attributes
/// #[derive(SomeIp)]
/// #[someip(
///     message_wrapper = true,
///     length_field_size = 2,
///     arrays_length_field_size = 2,
///     structs_length_field_size = 2,
///     strings_length_field_size = 2
/// )]
/// struct AnotherStruct {
///     #[someip(max_elements = 42)]
///     foo: Vec<f64>,
/// };
/// ```
/// Structs can also use the TLV encoding in which case every field must have an `id` which is in `0..=0xFFF` and the id
/// must be unique within the struct.
/// ```
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// struct AStruct {
///     #[someip(id = 0)]
///     foo: u32,
///     #[someip(id = 1)]
///     bar: Option<f64>,
/// };
/// ```
/// Either all fields must have an id or none:
/// ```compile_fail
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// struct AStruct {
///     #[someip(id = 0)]
///     foo: u32,
///     bar: f64,
/// };
/// ```
/// Options can only be used in TLV structs:
/// ```compile_fail
/// # use serde_someip::SomeIp;
/// #[derive(SomeIp)]
/// struct AStruct {
///     foo: u32,
///     bar: Option<f64>,
/// };
/// ```
///
/// # `treat_as`
/// You can use `treat_as` to workaround types that someip does not know.
/// For example typedefs or using the bytes crate:
/// ```
/// # use serde_someip::SomeIp;
/// use bytes::Bytes;
/// type ABunchOfInts = Vec<u32>;
///
/// #[derive(SomeIp)]
/// struct AStruct {
///     #[someip(treat_as = Vec<u32>, max_elements = 32)]
///     ints: ABunchOfInts,
///     #[someip(treat_as = [u8], max_elements = 1024)]
///     bytes: Bytes,
/// }
/// ```
/// Warning: If you lie to someip about what your type is your program will panic.
pub use serde_someip_derive::SomeIp;

#[cfg(feature = "derive")]
#[test]
fn test_derive_successes() {
    let t = trybuild::TestCases::new();
    t.pass("derive_tests/successes/*.rs")
}

#[cfg(feature = "derive")]
#[test]
fn test_derive_failures() {
    let t = trybuild::TestCases::new();
    t.compile_fail("derive_tests/failures/*.rs")
}
