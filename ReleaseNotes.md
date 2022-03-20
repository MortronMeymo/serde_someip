# Releases of `serde_someip`

## 0.2.0

- Breaking change: Remove `OVERWRITE_LENGTH_FIELD_SIZE` from `SomeIpOptions`. Since this is not really a global option this functionality was moved to the `SomeIp` trait on the de/serialized type.
- Add `arrays_length_field_size`, `structs_length_field_size`, `strings_length_field_size` to attributes for structs to mirror someip transformer properties from autosar. This replaces `OVERWRITE_LENGTH_FIELD_SIZE` from `SomeIpOptions`.
- Fix some typos in error messages
- Remove some accidentaly public methods from `SomeIpOptions` trait

## 0.1.3

- Fix panic when serializing a `None` for a field in a tlv struct

## 0.1.2

- Add doc for `treat_as` attribute on derive macro
- Add optional feature for supporting the `bytes` crate, allowing de/serializing from/to `Bytes`
- Add functions to serialize to an existing `Vec<u8>` or `BytesMut` which allows reusing them and reducing memory allocations
- Add convenience functions on `SomeIpOptions` trait to allow calling to/from methods through the options trait

## 0.1.1

- Add package metadata for `docs.rs`

## 0.1.0

- Initial release
