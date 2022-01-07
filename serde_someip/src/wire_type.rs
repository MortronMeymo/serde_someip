use super::error::{Error, Result};
use super::length_fields::LengthFieldSize;

use std::fmt::{Display, Formatter};

/// These are the wiretypes that are available
/// inside the tags of TLV serialized structs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WireType {
    /// A tag with this wiretype is followed by a one byte value.
    /// Used for bool, u8 and i8.
    OneByte,
    /// A tag with this wiretype is followed by a two byte value.
    /// Used for u16 and i16.
    TwoBytes,
    /// A tag with this wiretype is followed by a four byte value.
    /// Used for u32, i32 and f32.
    FourBytes,
    /// A tag with this wiretype is followed by a eight byte value.
    /// Used for u64, i64 and f64.
    EightBytes,
    /// A tag with this wiretype is followed by a length field followed by that many bytes of value.
    /// The size of the length field is determined from the config.
    /// Used for strings, sequences and structs.
    LengthDelimitedFromConfig,
    /// A tag with this wiretype is followed by a length field followed by that many bytes of value.
    /// The size of the length field is one byte.
    /// Used for strings, sequences and structs.
    LengthDelimitedOneByte,
    /// A tag with this wiretype is followed by a length field followed by that many bytes of value.
    /// The size of the length field is two bytes.
    /// Used for strings, sequences and structs.
    LengthDelimitedTwoBytes,
    /// A tag with this wiretype is followed by a length field followed by that many bytes of value.
    /// The size of the length field is four bytes.
    /// Used for strings, sequences and structs.
    LengthDelimitedFourBytes,
}

impl WireType {
    pub(crate) fn get_length_field_size(&self) -> Option<LengthFieldSize> {
        match self {
            WireType::LengthDelimitedOneByte => Some(LengthFieldSize::OneByte),
            WireType::LengthDelimitedTwoBytes => Some(LengthFieldSize::TwoBytes),
            WireType::LengthDelimitedFourBytes => Some(LengthFieldSize::FourBytes),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_fixed_size(&self) -> Option<usize> {
        match self {
            WireType::OneByte => Some(1),
            WireType::TwoBytes => Some(2),
            WireType::FourBytes => Some(4),
            WireType::EightBytes => Some(8),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn disect_tag(tag: u16) -> (WireType, u16) {
        (tag.into(), 0xFFF & tag)
    }

    #[inline]
    pub(crate) fn check(self, other: WireType) -> Result<()> {
        let matches = match self {
            WireType::OneByte => other == WireType::OneByte,
            WireType::TwoBytes => other == WireType::TwoBytes,
            WireType::FourBytes => other == WireType::FourBytes,
            WireType::EightBytes => other == WireType::EightBytes,
            _ => matches!(
                other,
                WireType::LengthDelimitedFromConfig
                    | WireType::LengthDelimitedOneByte
                    | WireType::LengthDelimitedTwoBytes
                    | WireType::LengthDelimitedFourBytes
            ),
        };
        if matches {
            Ok(())
        } else {
            Err(Error::InvalidWireType {
                expected: self.to_string(),
                actual: other.to_string(),
            })
        }
    }
}

impl Display for WireType {
    fn fmt(&self, formatter: &mut Formatter) -> std::fmt::Result {
        match self {
            WireType::OneByte => formatter.write_str("one byte(0)"),
            WireType::TwoBytes => formatter.write_str("two bytes(1)"),
            WireType::FourBytes => formatter.write_str("four bytes(2)"),
            WireType::EightBytes => formatter.write_str("eight bytes(3)"),
            WireType::LengthDelimitedFromConfig => {
                formatter.write_str("length delimited from config(4)")
            }
            WireType::LengthDelimitedOneByte => formatter.write_str("length delimited one byte(5)"),
            WireType::LengthDelimitedTwoBytes => {
                formatter.write_str("length delimited two bytes(6)")
            }
            WireType::LengthDelimitedFourBytes => {
                formatter.write_str("length delimited four bytes(7)")
            }
        }
    }
}

impl From<LengthFieldSize> for WireType {
    #[inline]
    fn from(v: LengthFieldSize) -> WireType {
        match v {
            LengthFieldSize::OneByte => WireType::LengthDelimitedOneByte,
            LengthFieldSize::TwoBytes => WireType::LengthDelimitedTwoBytes,
            LengthFieldSize::FourBytes => WireType::LengthDelimitedFourBytes,
        }
    }
}

impl From<WireType> for u16 {
    #[inline]
    fn from(v: WireType) -> u16 {
        match v {
            WireType::OneByte => 0x0000,
            WireType::TwoBytes => 0x1000,
            WireType::FourBytes => 0x2000,
            WireType::EightBytes => 0x3000,
            WireType::LengthDelimitedFromConfig => 0x4000,
            WireType::LengthDelimitedOneByte => 0x5000,
            WireType::LengthDelimitedTwoBytes => 0x6000,
            WireType::LengthDelimitedFourBytes => 0x7000,
        }
    }
}

impl From<u16> for WireType {
    #[inline]
    fn from(v: u16) -> WireType {
        let bits = (v >> 12) & 0x7;
        match bits {
            0 => WireType::OneByte,
            1 => WireType::TwoBytes,
            2 => WireType::FourBytes,
            3 => WireType::EightBytes,
            4 => WireType::LengthDelimitedFromConfig,
            5 => WireType::LengthDelimitedOneByte,
            6 => WireType::LengthDelimitedTwoBytes,
            7 => WireType::LengthDelimitedFourBytes,
            _ => panic!(),
        }
    }
}
