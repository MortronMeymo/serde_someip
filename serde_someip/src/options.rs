//! Provides the [SomeIpOptions] trait which is used to statically configure the
//! options to use during de/serializtaion.
//!
//! This is necessary since the someip standard does not fully define everything and
//! for example leaves the byte order open for projects to choose.

use super::error::{Error, Result};
use super::length_fields::LengthFieldSize;
use super::SomeIp;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use std::io::Read;

/// Its a ByteOrder enum, what do you expect?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    /// BigEndian, also sometimes called motorola byte order.
    ///
    /// The u16 0x1234 will be serialized as [0x12, 0x34]
    BigEndian,
    /// LittleEndian, also sometimes called intel byte order.
    ///
    /// The u16 0x1234 will be serialized as [0x34, 0x12]
    LittleEndian,
}

/// The supported string encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEncoding {
    /// Strings are UTF-8 encoded.
    ///
    /// This matches the internal string encoding of rust.
    Utf8,
    /// Strings are UTF-16 encoded and use the same byte order as primitve types.
    Utf16,
    /// Strings are UTF-16 encoded and are always encoded in little endian byte order.
    Utf16Le,
    /// Strings are UTF-16 encoded and are always encoded in big endian byte order.
    Utf16Be,
    /// Strings are ASCII encoded.
    ///
    /// Note that ASCII is a subset of UTF-8 so any ASCII string is also a valid UTF-8 string.
    Ascii,
}

impl StringEncoding {
    #[inline]
    pub(crate) fn is_utf16_variant(&self) -> bool {
        matches!(
            self,
            StringEncoding::Utf16 | StringEncoding::Utf16Le | StringEncoding::Utf16Be
        )
    }
}

/// How the serializer should pick LengthFieldSizes in TLV encoded structs.
///
/// Non TLV structs must always use the configured length field size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthFieldSizeSelection {
    /// The length field size will be selected as configured if that is possible.
    /// If the actual data is longer than can fit into the configured length field size
    /// the serializer will pick the smallest length field size that can hold the length.
    AsConfigured,
    /// The serializer will always pick the smallest length field size that can hold the length.
    Smallest,
}

/// What the deserializer should do when it encounters a string or sequence with too much data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOnTooMuchData {
    /// The deserializer will issue a [TooMuchData](super::error::Error::TooMuchData) error.
    Fail,
    /// The deserializer will silently discard the extra data.
    ///
    /// Note that when dealing with strings this may cause a [CannotCodeString](super::error::Error::CannotCodeString) error.
    Discard,
    /// The deserializer will simply keep the extra data.
    ///
    /// Note that this means [String]s and [Vec]s can grow beyond expected sizes.
    ///
    /// Also this options is technically not complient with the someip autosar standard.
    Keep,
}

/// The options to use for de/serialization.
///
/// It is expected that these will be almost always identical, since they are usually defined once
/// either per project or even per vehicle OEM.
///
/// This trait also provides some convenience functions for serializing/deserializing:
/// ```
/// # use serde_someip::options::ExampleOptions;
/// use serde_someip::{SomeIpOptions, to_vec, from_slice};
///
/// let manually = to_vec::<ExampleOptions, _>(&42u32).unwrap();
/// let convenience = ExampleOptions::to_vec(&42u32).unwrap();
///
/// assert_eq!(manually, convenience);
///
/// let manually: u32 =  from_slice::<ExampleOptions, _>(&manually).unwrap();
/// let convenience: u32 = ExampleOptions::from_slice(&convenience).unwrap();
///
/// assert_eq!(manually, convenience);
/// ```
pub trait SomeIpOptions {
    /// The byte order to use
    const BYTE_ORDER: ByteOrder = ByteOrder::BigEndian;

    /// If strings must start with a BOM.
    ///
    /// Setting this to `true` while using [Ascii](StringEncoding::Ascii) is an error, since the BOM char is not part of the ASCII char set.
    ///
    /// If true and the encoding is any utf-16 encoding the deserializer will dynamically determine the byte order of strings based on the BOM.
    const STRING_WITH_BOM: bool = false;
    /// The encoding used by strings
    const STRING_ENCODING: StringEncoding = StringEncoding::Utf8;
    /// If strings must end with a null terminator.
    const STRING_WITH_TERMINATOR: bool = false;

    /// The default length field size to use if the type does not define one.
    const DEFAULT_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = Some(LengthFieldSize::FourBytes);

    /// Should the serializer output the legacy wire type for length delimited fields?
    ///
    /// By default wiretypes 5(length delimited one byte), 6(length delimited two bytes) or
    /// 7(length delimited four bytes) are used for length delimited fields
    /// by enabling this option wiretype 4(length delimited from config) will be used
    /// if the length fields size equals the one that was statically configured.
    const SERIALIZER_USE_LEGACY_WIRE_TYPE: bool = false;

    /// How the size of the length field size should be selected, see [LengthFieldSizeSelection].
    const SERIALIZER_LENGTH_FIELD_SIZE_SELECTION: LengthFieldSizeSelection =
        LengthFieldSizeSelection::Smallest;

    /// Should the deserializer issue a error if a bool is expected and a u8 >  1 is encountered?
    ///
    /// Usually only 0 and 1 are allowed for booleans but misbehaving implementations may
    /// send larger values. With strict booleans such values lead to [InvalidBool](super::Error::InvalidBool) in lenient mode
    /// they are interpreted as true.
    const DESERIALIZER_STRICT_BOOL: bool = false;

    /// How the deserializer treats strings or sequnces with too much data, see [ActionOnTooMuchData].
    const DESERIALIZER_ACTION_ON_TOO_MUCH_DATA: ActionOnTooMuchData = ActionOnTooMuchData::Discard;

    /// Verifies that the string encoding is valid.
    ///
    /// # Panics
    /// Panics if the string encoding is invalid.
    #[inline]
    fn verify_string_encoding() {
        if Self::STRING_ENCODING == StringEncoding::Ascii && Self::STRING_WITH_BOM {
            panic!("Encoding is ASCII with BOM which is impossible");
        }
    }

    /// Convenience wrapper for [super::from_reader]
    #[inline]
    fn from_reader<T, Reader>(reader: Reader, len: usize) -> Result<T>
    where
        T: DeserializeOwned + SomeIp + ?Sized,
        Reader: Read,
    {
        super::from_reader::<Self, T, _>(reader, len)
    }

    /// Convenience wrapper for [super::from_slice]
    #[inline]
    fn from_slice<'a, T>(slice: &'a [u8]) -> Result<T>
    where
        T: Deserialize<'a> + SomeIp + ?Sized,
    {
        super::from_slice::<'a, Self, T>(slice)
    }

    #[cfg(feature = "bytes")]
    /// Convenience wrapper for [super::from_bytes]
    ///
    /// *Only available with the `bytes` feature.*
    #[inline]
    fn from_bytes<T>(data: bytes::Bytes) -> Result<T>
    where
        T: DeserializeOwned + SomeIp + ?Sized,
    {
        super::from_bytes::<Self, T>(data)
    }

    /// Convenience wrapper for [super::to_vec]
    #[inline]
    fn to_vec<T>(value: &T) -> Result<Vec<u8>>
    where
        T: Serialize + SomeIp,
    {
        super::to_vec::<Self, _>(value)
    }

    /// Convenience wrapper for [super::append_to_vec]
    #[inline]
    fn append_to_vec<T>(value: &T, vec: &mut Vec<u8>) -> Result<()>
    where
        T: Serialize + SomeIp,
    {
        super::append_to_vec::<Self, _>(value, vec)
    }

    #[cfg(feature = "bytes")]
    /// Convenience wrapper for [super::to_bytes]
    ///
    /// *Only available with the `bytes` feature.*
    #[inline]
    fn to_bytes<T>(value: &T) -> Result<bytes::Bytes>
    where
        T: Serialize + SomeIp,
    {
        super::to_bytes::<Self, _>(value)
    }

    #[cfg(feature = "bytes")]
    /// Convenience wrapper for [super::append_to_bytes]
    ///
    /// *Only available with the `bytes` feature.*
    fn append_to_bytes<T>(value: &T, bytes: &mut bytes::BytesMut) -> Result<()>
    where
        T: Serialize + SomeIp,
    {
        super::append_to_bytes::<Self, _>(value, bytes)
    }
}

#[inline]
pub(crate) fn apply_defaults<T: SomeIpOptions + ?Sized>(
    from_type: Option<LengthFieldSize>,
) -> Option<LengthFieldSize> {
    from_type.or(T::DEFAULT_LENGTH_FIELD_SIZE)
}

#[inline]
pub(crate) fn select_length_field_size<T: SomeIpOptions + ?Sized>(
    configured: LengthFieldSize,
    len: usize,
    is_in_tlv_struct: bool,
) -> Result<LengthFieldSize> {
    let minimum_needed = LengthFieldSize::minimum_length_for(len);
    if is_in_tlv_struct {
        match T::SERIALIZER_LENGTH_FIELD_SIZE_SELECTION {
            LengthFieldSizeSelection::AsConfigured => {
                if minimum_needed <= configured {
                    Ok(configured)
                } else {
                    Ok(minimum_needed)
                }
            }
            LengthFieldSizeSelection::Smallest => Ok(minimum_needed),
        }
    } else if configured < minimum_needed {
        Err(Error::TooLong {
            actual_length: len,
            length_field_size: configured,
        })
    } else {
        Ok(configured)
    }
}

/// A struct that implements [SomeIpOptions] without overwriting any defaults to quickly get started.
pub struct ExampleOptions;
impl SomeIpOptions for ExampleOptions {}

#[cfg(test)]
pub(crate) mod test {
    pub struct LittleEndianOptions;
    impl super::SomeIpOptions for LittleEndianOptions {
        const BYTE_ORDER: super::ByteOrder = super::ByteOrder::LittleEndian;
    }
}
