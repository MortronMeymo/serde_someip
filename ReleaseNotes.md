# Releases of `serde_someip`

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
