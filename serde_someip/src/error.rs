//! This module contains the [Error] type used by the serializer and deserializer of this crate.

use super::length_fields::LengthFieldSize;

use serde::{de, ser};

use std::fmt::{Display, Formatter};

/// The error type used by both the serializer and deserializer.
#[derive(Debug)]
pub enum Error {
    /// Custom errors returned by serde.
    Message(String),

    /// Expected a bool but found some u8 > 1.
    /// This error is only possible if [SomeIpOptions::DESERIALIZER_STRICT_BOOL](super::options::SomeIpOptions::DESERIALIZER_STRICT_BOOL) is `true`.
    InvalidBool(u8),
    /// The deserialized raw value does not map to any enum variant.
    /// Usually this indicates either a malformed message or
    /// an incompatible Enum definition.
    InvalidEnumValue {
        /// The value that was received. Allready converted to [String].
        value: String,
        /// The name of the enum that we attempted to deserialize.
        name: &'static str,
    },
    /// The deserialized wiretype is incompatible with the expected one.
    /// This can only occure if TLV structs are used.
    InvalidWireType {
        ///The wiretype that was expected.
        expected: String,
        ///The actually encountered wiretype.
        actual: String,
    },
    /// A string could not be encoded or decoded.
    /// The [String] parameter gives more info as to what went wrong.
    CannotCodeString(String),

    /// A string or sequence with a minimum size (min_size for strings and min_elements for sequences)
    /// was deserialized and contained less data than the minimum size.
    NotEnoughData {
        /// The minimum size required.
        /// For strings this is in bytes for sequences in elements.
        min: usize,
        ///The actual size recived.
        /// For strings this is in bytes for sequences in elements.
        actual: usize,
    },
    /// A string or sequence was deserialized and contained more data than was allowed
    /// (max_size for stirngs and max_elements for sequences).
    /// This error is only possible if [SomeIpOptions::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA](super::options::SomeIpOptions::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA) is [Fail](super::options::ActionOnTooMuchData::Fail)
    TooMuchData {
        /// The maximum size allowed.
        /// For strings this is in bytes for sequences in elements.
        max: usize,
        /// THe actual size found, maybe zero if it is impossible to determine the actual number of elements.
        /// For strings this is in bytes for sequences in elements.
        actual: usize,
    },

    /// Deserializing the message requires reading beyond the end of the serialized data.
    /// This error is also emitted if a length field inside the message indicates `x` bytes
    /// but parsing the data after the length field takes more than `x` bytes.
    TooShort,
    /// Serialized more data than fits into to specified length field,
    /// for example trying to write a 300 byte string with a one byte length field.
    TooLong {
        /// The actual length of the serialized data.
        actual_length: usize,
        /// The length field size that must be used.
        length_field_size: LengthFieldSize,
    },
    /// After deserializing there are still some bytes left over at the end of the serialized data.
    /// The parameter indicates how many bytes are left over.
    /// This is also triggered if a length field indicates `x` bytes but parsing consumed less than `x` bytes.
    NotAllBytesConsumed(usize),

    /// Some [std::io::Error] occured during de/serialization.
    /// This error is currently only possible if [from_reader](super::from_reader) is used.
    IoError(std::io::Error),
}

/// The result type returned from serializer and deserializer
/// using [Error] as the error type.
pub type Result<T> = std::result::Result<T, Error>;

impl From<std::str::Utf8Error> for Error {
    fn from(v: std::str::Utf8Error) -> Error {
        Error::CannotCodeString(format!("Invalid utf8: {}", v))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(v: std::string::FromUtf8Error) -> Error {
        Error::CannotCodeString(format!("Invalid utf8: {}", v))
    }
}

impl From<std::char::DecodeUtf16Error> for Error {
    fn from(v: std::char::DecodeUtf16Error) -> Error {
        Error::CannotCodeString(format!("Invalid utf16: {}", v))
    }
}

impl From<std::io::Error> for Error {
    fn from(v: std::io::Error) -> Error {
        Error::IoError(v)
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter) -> std::fmt::Result {
        match self {
            Error::Message(s) => formatter.write_str(s),
            Error::InvalidBool(v) => {
                formatter.write_fmt(format_args!("Invalid value for bool: {}", v))
            }
            Error::InvalidEnumValue { value, name } => formatter.write_fmt(format_args!(
                "Invalid enum value {} for enum {}",
                value, name
            )),
            Error::InvalidWireType { expected, actual } => formatter.write_fmt(format_args!(
                "Invalid wire type expected {} but got {}",
                expected, actual
            )),
            Error::NotEnoughData { min, actual } => formatter.write_fmt(format_args!(
                "Not enough data needed {} but got {}",
                min, actual
            )),
            Error::TooMuchData { max, actual } => formatter.write_fmt(format_args!(
                "Too much data needed {} but got {}",
                max, actual
            )),
            Error::TooShort => formatter.write_str("Ran out of bytes before the end was reached"),
            Error::TooLong {
                actual_length,
                length_field_size,
            } => formatter.write_fmt(format_args!(
                "Too much data for length delimited section with length_field_size={}, len={}",
                length_field_size, actual_length
            )),
            Error::NotAllBytesConsumed(v) => {
                formatter.write_fmt(format_args!("Not all bytes were consuemd, {} were left", v))
            }
            Error::CannotCodeString(v) => {
                formatter.write_fmt(format_args!("Cannot en/decode string: {}", v))
            }
            Error::IoError(v) => formatter.write_fmt(format_args!("Io Error: {}", v)),
        }
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}
