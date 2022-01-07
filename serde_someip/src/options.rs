//! Provides the [SomeIpOptions] trait which is used to statically configure the
//! options to use during de/serializtaion.
//!
//! This is necessary since the someip standard does not fully define everything and
//! for example leaves the byte order open for projects to choose.

use super::error::{Error, Result};
use super::length_fields::LengthFieldSize;

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
    /// Strings are UTF-16 encoded.
    Utf16,
    /// Strings are ASCII encoded.
    ///
    /// Note that ASCII is a subset of UTF-8 so any ASCII string is also a valid UTF-8 string.
    Ascii,
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
pub trait SomeIpOptions {
    /// The byte order to use
    const BYTE_ORDER: ByteOrder = ByteOrder::BigEndian;

    /// If strings must start with a BOM.
    ///
    /// Setting this to `true` while using [Ascii](StringEncoding::Ascii) is an error, since the BOM char is not part of the ASCII char set.
    ///
    /// If true and the encoding is [Utf16](StringEncoding::Utf16) the deserializer will dynamically determine the byte order of strings.
    const STRING_WITH_BOM: bool = false;
    /// The encoding used by strings
    const STRING_ENCODING: StringEncoding = StringEncoding::Utf8;
    /// If strings must end with a null terminator.
    const STRING_WITH_TERMINATOR: bool = false;

    /// The default length field size to use if the type does not define one.
    const DEFAULT_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = Some(LengthFieldSize::FourBytes);

    /// The length field size whith which to overwrite all length field sizes.
    ///
    /// This is very commonly used if TLV structs are being used.
    /// You can use [create_length_field_overwrites] to quickly create all possible versions of your options regarding this field.
    const OVERWRITE_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = None;

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

    #[doc(hidden)]
    #[inline]
    fn overwrite_length_field_size(from_type: Option<LengthFieldSize>) -> Option<LengthFieldSize> {
        if Self::OVERWRITE_LENGTH_FIELD_SIZE.is_some() {
            Self::OVERWRITE_LENGTH_FIELD_SIZE
        } else if from_type.is_some() {
            from_type
        } else {
            Self::DEFAULT_LENGTH_FIELD_SIZE
        }
    }

    #[doc(hidden)]
    #[inline]
    fn select_length_field_size(
        configured: LengthFieldSize,
        len: usize,
        is_in_tlv_struct: bool,
    ) -> Result<LengthFieldSize> {
        let minimum_needed = LengthFieldSize::minimum_length_for(len);
        if is_in_tlv_struct {
            match Self::SERIALIZER_LENGTH_FIELD_SIZE_SELECTION {
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
}

/// This macro can be used to quickly provide the different options possible for [OVERWRITE_LENGTH_FIELD_SIZE](SomeIpOptions::OVERWRITE_LENGTH_FIELD_SIZE).
///
/// ```
/// # use serde_someip::length_fields::LengthFieldSize;
/// use serde_someip::options::*;
/// use serde_someip::create_length_field_overwrites;
///
/// struct MyOptions;
/// impl SomeIpOptions for MyOptions {
///     //must be None for the macro to work correctly
///     const OVERWRITE_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = None;
///     // overwrite other constants as needed
/// }
///
/// create_length_field_overwrites!(
///     MyOptions;
///     struct MyOptionsOverwriteOne;
///     struct MyOptionsOverwriteTwo;
///     struct MyOptionsOverwriteFour;
/// );
///
/// assert_eq!(MyOptionsOverwriteTwo::OVERWRITE_LENGTH_FIELD_SIZE, Some(LengthFieldSize::TwoBytes));
/// ```
#[macro_export]
macro_rules! create_length_field_overwrites {
    (@create_one $orig:ident,  $(#[$outer:meta])* $vis:vis $newType:ident $newName:ident, $lengthFieldSize:expr) => {
        $(#[$outer])*
        $vis $newType $newName;
        impl SomeIpOptions for $newName {
            const BYTE_ORDER: ByteOrder = $orig::BYTE_ORDER;
            const STRING_WITH_BOM: bool = $orig::STRING_WITH_BOM;
            const STRING_ENCODING: StringEncoding = $orig::STRING_ENCODING;
            const STRING_WITH_TERMINATOR: bool = $orig::STRING_WITH_TERMINATOR;
            const DEFAULT_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = $orig::DEFAULT_LENGTH_FIELD_SIZE;
            const OVERWRITE_LENGTH_FIELD_SIZE: Option<LengthFieldSize> = $lengthFieldSize;
            const SERIALIZER_USE_LEGACY_WIRE_TYPE: bool = $orig::SERIALIZER_USE_LEGACY_WIRE_TYPE;
            const SERIALIZER_LENGTH_FIELD_SIZE_SELECTION: LengthFieldSizeSelection = $orig::SERIALIZER_LENGTH_FIELD_SIZE_SELECTION;
            const DESERIALIZER_STRICT_BOOL: bool = $orig::DESERIALIZER_STRICT_BOOL;
            const DESERIALIZER_ACTION_ON_TOO_MUCH_DATA: ActionOnTooMuchData = $orig::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA;
        }
    };

    ($orig:ident;  $(#[$outer1:meta])* $vis1:vis $type1:ident $name1:ident;  $(#[$outer2:meta])* $vis2:vis $type2:ident $name2:ident;  $(#[$outer3:meta])* $vis3:vis $type3:ident $name3:ident;) => {
        create_length_field_overwrites!(@create_one $orig, $(#[$outer1])* $vis1 $type1 $name1, Some(LengthFieldSize::OneByte));
        create_length_field_overwrites!(@create_one $orig, $(#[$outer2])* $vis2 $type2 $name2, Some(LengthFieldSize::TwoBytes));
        create_length_field_overwrites!(@create_one $orig, $(#[$outer3])* $vis3 $type3 $name3, Some(LengthFieldSize::FourBytes));
    };
}

/// A struct that implements [SomeIpOptions] without overwriting any defaults to quickly get started.
pub struct ExampleOptions;
impl SomeIpOptions for ExampleOptions {}
create_length_field_overwrites!(
    ExampleOptions;
    /// A struct that implements [SomeIpOptions] without overwriting any defaults exepct for [OVERWRITE_LENGTH_FIELD_SIZE](SomeIpOptions::OVERWRITE_LENGTH_FIELD_SIZE) to [OneByte](super::length_fields::LengthFieldSize::OneByte).
    pub struct ExampleOptionsOverwriteOne;
    /// A struct that implements [SomeIpOptions] without overwriting any defaults exepct for [OVERWRITE_LENGTH_FIELD_SIZE](SomeIpOptions::OVERWRITE_LENGTH_FIELD_SIZE) to [TwoBytes](super::length_fields::LengthFieldSize::TwoBytes).
    pub struct ExampleOptionsOverwriteTwo;
    /// A struct that implements [SomeIpOptions] without overwriting any defaults exepct for [OVERWRITE_LENGTH_FIELD_SIZE](SomeIpOptions::OVERWRITE_LENGTH_FIELD_SIZE) to [FourBytes](super::length_fields::LengthFieldSize::FourBytes).
    pub struct ExampleOptionsOverwriteFour;
);

#[cfg(test)]
pub(crate) mod test {
    pub struct LittleEndianOptions;
    impl super::SomeIpOptions for LittleEndianOptions {
        const BYTE_ORDER: super::ByteOrder = super::ByteOrder::LittleEndian;
    }
}
