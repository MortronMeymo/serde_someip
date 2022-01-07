//! Provides the [SomeIp] trait and various related types. Consider using the `derive`
//! feature in which case you can ignore this module.

use super::error::Result;
use super::length_fields::LengthFieldSize;
use super::options::SomeIpOptions;
use super::wire_type::WireType;
use std::fmt::{Display, Formatter};

pub(crate) trait SomeIpSize {
    fn wanted_length_field<Options: SomeIpOptions>(
        &self,
        is_in_tlv_struct: bool,
    ) -> Result<Option<LengthFieldSize>>;
    fn is_const_size(&self) -> bool;
    fn max_len<Options: SomeIpOptions>(&self, is_in_tlv_struct: bool) -> Result<usize>;
}

/// All primitives defined by SomeIp.
#[derive(Debug, PartialEq, Eq)]
pub enum SomeIpPrimitive {
    /// A bool.
    Bool,
    /// An u8.
    U8,
    /// An u16.
    U16,
    /// An u32.
    U32,
    /// An u64.
    U64,
    /// An i8.
    I8,
    /// An i16.
    I16,
    /// An i32.
    I32,
    /// An i64.
    I64,
    /// An f32.
    F32,
    /// An f64.
    F64,
}

impl SomeIpPrimitive {
    #[inline]
    fn get_wire_type(&self) -> WireType {
        match self {
            SomeIpPrimitive::Bool | SomeIpPrimitive::U8 | SomeIpPrimitive::I8 => WireType::OneByte,
            SomeIpPrimitive::U16 | SomeIpPrimitive::I16 => WireType::TwoBytes,
            SomeIpPrimitive::U32 | SomeIpPrimitive::I32 | SomeIpPrimitive::F32 => {
                WireType::FourBytes
            }
            SomeIpPrimitive::U64 | SomeIpPrimitive::I64 | SomeIpPrimitive::F64 => {
                WireType::EightBytes
            }
        }
    }

    #[inline]
    pub(crate) fn get_len(&self) -> usize {
        match self {
            SomeIpPrimitive::Bool | SomeIpPrimitive::U8 | SomeIpPrimitive::I8 => 1,
            SomeIpPrimitive::U16 | SomeIpPrimitive::I16 => 2,
            SomeIpPrimitive::U32 | SomeIpPrimitive::I32 | SomeIpPrimitive::F32 => 4,
            SomeIpPrimitive::U64 | SomeIpPrimitive::I64 | SomeIpPrimitive::F64 => 8,
        }
    }
}

impl Display for SomeIpPrimitive {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            SomeIpPrimitive::Bool => f.write_str("bool"),
            SomeIpPrimitive::U8 => f.write_str("u8"),
            SomeIpPrimitive::U16 => f.write_str("u16"),
            SomeIpPrimitive::U32 => f.write_str("u32"),
            SomeIpPrimitive::U64 => f.write_str("u64"),
            SomeIpPrimitive::I8 => f.write_str("i8"),
            SomeIpPrimitive::I16 => f.write_str("i16"),
            SomeIpPrimitive::I32 => f.write_str("i32"),
            SomeIpPrimitive::I64 => f.write_str("i64"),
            SomeIpPrimitive::F32 => f.write_str("f32"),
            SomeIpPrimitive::F64 => f.write_str("f64"),
        }
    }
}

/// An enum that can hold the serialized value
/// for any possible enum supported by someip.
#[derive(Debug, PartialEq, Eq)]
pub enum SomeIpEnumValue {
    /// For enums with raw_type = u8.
    U8(u8),
    /// For enums with raw_type = u16.
    U16(u16),
    /// For enums with raw_type = u32.
    U32(u32),
    /// For enums with raw_type = u64.
    U64(u64),
    /// For enums with raw_type = i8.
    I8(i8),
    /// For enums with raw_type = i16.
    I16(i16),
    /// For enums with raw_type = i32.
    I32(i32),
    /// For enums with raw_type = i64.
    I64(i64),
}

impl SomeIpEnumValue {
    fn matches(&self, other: &SomeIpPrimitive) -> bool {
        match self {
            SomeIpEnumValue::U8(_) => {
                matches!(other, SomeIpPrimitive::U8)
            }
            SomeIpEnumValue::U16(_) => {
                matches!(other, SomeIpPrimitive::U16)
            }
            SomeIpEnumValue::U32(_) => {
                matches!(other, SomeIpPrimitive::U32)
            }
            SomeIpEnumValue::U64(_) => {
                matches!(other, SomeIpPrimitive::U64)
            }
            SomeIpEnumValue::I8(_) => {
                matches!(other, SomeIpPrimitive::I8)
            }
            SomeIpEnumValue::I16(_) => {
                matches!(other, SomeIpPrimitive::I16)
            }
            SomeIpEnumValue::I32(_) => {
                matches!(other, SomeIpPrimitive::I32)
            }
            SomeIpEnumValue::I64(_) => {
                matches!(other, SomeIpPrimitive::I64)
            }
        }
    }

    fn display_type(&self) -> impl Display {
        match self {
            SomeIpEnumValue::U8(_) => "u8",
            SomeIpEnumValue::U16(_) => "u16",
            SomeIpEnumValue::U32(_) => "u32",
            SomeIpEnumValue::U64(_) => "u64",
            SomeIpEnumValue::I8(_) => "i8",
            SomeIpEnumValue::I16(_) => "i16",
            SomeIpEnumValue::I32(_) => "i32",
            SomeIpEnumValue::I64(_) => "i64",
        }
    }

    pub(crate) fn display_value(&self) -> String {
        match self {
            SomeIpEnumValue::U8(v) => v.to_string(),
            SomeIpEnumValue::U16(v) => v.to_string(),
            SomeIpEnumValue::U32(v) => v.to_string(),
            SomeIpEnumValue::U64(v) => v.to_string(),
            SomeIpEnumValue::I8(v) => v.to_string(),
            SomeIpEnumValue::I16(v) => v.to_string(),
            SomeIpEnumValue::I32(v) => v.to_string(),
            SomeIpEnumValue::I64(v) => v.to_string(),
        }
    }
}

/// All the data needed to de/serialize a enum.
#[derive(Debug, PartialEq, Eq)]
pub struct SomeIpEnum {
    /// The name of the enum. Usefull for debuging messages.
    pub name: &'static str,
    /// The values that the enum has as a tuple of `(VariantName, Value)`.
    /// All SomeIpEnumValues must match [raw_type](SomeIpEnum::raw_type).
    pub values: &'static [(&'static str, SomeIpEnumValue)],
    /// The primitive type used for de/serialization.
    pub raw_type: SomeIpPrimitive,
}

impl SomeIpEnum {
    /// Retrieves the [raw_type](SomeIpEnum::raw_type) of this enum.
    #[inline]
    pub fn get_raw_type(&self) -> &SomeIpPrimitive {
        &self.raw_type
    }

    /// Maps the given name onto the value used for serialization.
    #[inline]
    pub fn name_to_value(&self, name: &'static str) -> Option<&'static SomeIpEnumValue> {
        self.values
            .iter()
            .find_map(|(n, v)| if name == *n { Some(v) } else { None })
    }

    /// Maps the given vale onto the variant name, essentially the inverse of [name_to_value](SomeIpEnum::name_to_value).
    #[inline]
    pub fn value_to_name(&self, value: &SomeIpEnumValue) -> Option<&'static str> {
        self.values
            .iter()
            .find_map(|(n, v)| if value == v { Some(*n) } else { None })
    }
}

/// All the data needed to de/serialize a string, except for encoding that comes from [SomeIpOptions](super::options::SomeIpOptions).
#[derive(Debug, PartialEq, Eq)]
pub struct SomeIpString {
    /// The maximum size of the string in bytes *after* encoding.
    pub max_size: usize,
    /// The minimum size of the string in bytes *after* encoding.
    pub min_size: usize,
    /// The length field size to use for this string.
    pub length_field_size: Option<LengthFieldSize>,
}

impl SomeIpSize for SomeIpString {
    fn wanted_length_field<Options: SomeIpOptions>(
        &self,
        is_in_tlv_struct: bool,
    ) -> Result<Option<LengthFieldSize>> {
        if !self.is_const_size() || is_in_tlv_struct {
            let size = Options::overwrite_length_field_size(self.length_field_size);
            if size.is_none() {
                panic!("Required a length field size but none was specified");
            }
            Ok(size)
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn is_const_size(&self) -> bool {
        self.min_size == self.max_size
    }

    fn max_len<Options: SomeIpOptions>(&self, is_in_tlv_struct: bool) -> Result<usize> {
        let size = self.wanted_length_field::<Options>(is_in_tlv_struct)?;
        if let Some(size) = size {
            let size = Options::select_length_field_size(size, self.max_size, is_in_tlv_struct)?;
            Ok(self.max_size + usize::from(size))
        } else {
            Ok(self.max_size)
        }
    }
}

/// All the data needed to de/serialize a sequence.
#[derive(Debug, PartialEq, Eq)]
pub struct SomeIpSequence {
    /// The maximum number of elements this sequence may have.
    pub max_elements: usize,
    /// The minimum number of elements this sequence must have.
    pub min_elements: usize,
    /// The type of the elements inside this sequence.
    pub element_type: &'static SomeIpType,
    /// The length field size to use for this sequence.
    pub length_field_size: Option<LengthFieldSize>,
}

/// All the data needed to de/serialize one field of a [SomeIpStruct].
#[derive(Debug, PartialEq, Eq)]
pub struct SomeIpField {
    /// The name of the field.
    pub name: &'static str,
    /// The tlv id of this field. Must be `Some(...)` for TLV structs and `None` for non TLV structs.
    pub id: Option<u16>,
    /// The type of this field.
    pub field_type: &'static SomeIpType,
}

/// All the data needed to de/serialize a struct.
#[derive(Debug, PartialEq, Eq)]
pub struct SomeIpStruct {
    /// The name of the struct. Usefull for debugging messages.
    pub name: &'static str,
    /// All fields of this struct.
    pub fields: &'static [SomeIpField],
    /// Is this struct a TLV struct?
    pub uses_tlv_serialization: bool,
    /// Is this struct a message wrapper?
    /// A message wrapper struct is used to serialize the parameters or return values of a someip function
    /// or the data of one event, as such it never has a length field, since that information must be taken from the SomeIp header.
    pub is_message_wrapper: bool,
    /// The length field size to use for this struct.
    pub length_field_size: Option<LengthFieldSize>,
}

impl SomeIpStruct {
    /// Retrieves the field with the given name.
    #[inline]
    pub fn field_by_name(&self, name: &'static str) -> Option<&SomeIpField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Retrieves the field with the given id.
    #[inline]
    pub fn field_by_id(&self, id: u16) -> Option<&SomeIpField> {
        self.fields.iter().find(|f| f.id == Some(id))
    }

    /// Retrieves count of fields for this struct.
    #[inline]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Accessor for [SomeIpStruct::uses_tlv_serialization].
    #[inline]
    pub fn uses_tlv(&self) -> bool {
        self.uses_tlv_serialization
    }
}

/// All the data needed to de/serialize any tpye supported by someip.
#[derive(Debug, PartialEq, Eq)]
pub enum SomeIpType {
    /// Indicates a primitve.
    Primitive(SomeIpPrimitive),
    /// Indicates an enum.
    Enum(SomeIpEnum),
    /// Indicates a string.
    String(SomeIpString),
    /// Indicates a sequence.
    Sequence(SomeIpSequence),
    /// Indicates a struct.
    Struct(SomeIpStruct),
}

impl SomeIpType {
    #[inline]
    pub(crate) fn get_wire_type(&self) -> WireType {
        match self {
            SomeIpType::Primitive(prim) => prim.get_wire_type(),
            SomeIpType::Enum(e) => e.raw_type.get_wire_type(),
            _ => WireType::LengthDelimitedFromConfig,
        }
    }
}

impl Display for SomeIpType {
    fn fmt(&self, formatter: &mut Formatter) -> std::fmt::Result {
        match self {
            SomeIpType::Primitive(p) => formatter.write_fmt(format_args!("Primitve({})", p)),
            SomeIpType::Enum(e) => formatter.write_fmt(format_args!("Enum({})", e.name)),
            SomeIpType::String(_) => formatter.write_str("String"),
            SomeIpType::Sequence(_) => formatter.write_str("Sequence"),
            SomeIpType::Struct(s) => formatter.write_fmt(format_args!("Struct({})", s.name)),
        }
    }
}

impl SomeIpSize for SomeIpSequence {
    fn wanted_length_field<Options: SomeIpOptions>(
        &self,
        is_in_tlv_struct: bool,
    ) -> Result<Option<LengthFieldSize>> {
        if !self.is_const_size() || is_in_tlv_struct {
            let size = Options::overwrite_length_field_size(self.length_field_size);
            if size.is_none() {
                panic!("Required a length field size but none was specified");
            }
            Ok(size)
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn is_const_size(&self) -> bool {
        self.min_elements == self.max_elements && self.element_type.is_const_size()
    }

    fn max_len<Options: SomeIpOptions>(&self, is_in_tlv_struct: bool) -> Result<usize> {
        let size = self.wanted_length_field::<Options>(is_in_tlv_struct)?;
        let len = self.max_elements * self.element_type.max_len::<Options>(false)?;
        if let Some(size) = size {
            let size = Options::select_length_field_size(size, len, is_in_tlv_struct)?;
            Ok(len + usize::from(size))
        } else {
            Ok(len)
        }
    }
}

impl SomeIpSize for SomeIpStruct {
    fn wanted_length_field<Options: SomeIpOptions>(
        &self,
        is_in_tlv_struct: bool,
    ) -> Result<Option<LengthFieldSize>> {
        if self.is_message_wrapper {
            return Ok(None);
        }

        let needs_length_field = is_in_tlv_struct || self.uses_tlv();
        let mut size = None;
        if needs_length_field || self.length_field_size.is_some() {
            size = Options::overwrite_length_field_size(self.length_field_size);
        }
        if needs_length_field && size.is_none() {
            panic!(
                "Required a length field size for struct {} but none was specified",
                self.name
            );
        }
        Ok(size)
    }

    #[inline]
    fn is_const_size(&self) -> bool {
        !self.uses_tlv() && self.fields.iter().all(|f| f.field_type.is_const_size())
    }

    fn max_len<Options: SomeIpOptions>(&self, is_in_tlv_struct: bool) -> Result<usize> {
        let size = self.wanted_length_field::<Options>(is_in_tlv_struct)?;
        let mut len = 0;
        for f in self.fields {
            len += f
                .field_type
                .max_len::<Options>(self.uses_tlv_serialization)?;
        }
        if let Some(size) = size {
            let size = Options::select_length_field_size(size, len, is_in_tlv_struct)?;
            Ok(len + usize::from(size))
        } else {
            Ok(len)
        }
    }
}

impl SomeIpSize for SomeIpType {
    #[inline]
    fn wanted_length_field<Options: SomeIpOptions>(
        &self,
        is_in_tlv_struct: bool,
    ) -> Result<Option<LengthFieldSize>> {
        match self {
            SomeIpType::Primitive(_) | SomeIpType::Enum(_) => Ok(None),
            SomeIpType::String(s) => s.wanted_length_field::<Options>(is_in_tlv_struct),
            SomeIpType::Sequence(s) => s.wanted_length_field::<Options>(is_in_tlv_struct),
            SomeIpType::Struct(s) => s.wanted_length_field::<Options>(is_in_tlv_struct),
        }
    }

    #[inline]
    fn is_const_size(&self) -> bool {
        match self {
            SomeIpType::Primitive(_) | SomeIpType::Enum(_) => true,
            SomeIpType::String(s) => s.is_const_size(),
            SomeIpType::Sequence(s) => s.is_const_size(),
            SomeIpType::Struct(s) => s.is_const_size(),
        }
    }

    #[inline]
    fn max_len<Options: SomeIpOptions>(&self, is_in_tlv_struct: bool) -> Result<usize> {
        match self {
            SomeIpType::Primitive(prim) => Ok(prim.get_len()),
            SomeIpType::Enum(e) => Ok(e.raw_type.get_len()),
            SomeIpType::String(s) => s.max_len::<Options>(is_in_tlv_struct),
            SomeIpType::Sequence(s) => s.max_len::<Options>(is_in_tlv_struct),
            SomeIpType::Struct(s) => s.max_len::<Options>(is_in_tlv_struct),
        }
    }
}

/// A trait that can be used to perform some verification on SomeIpTypes.
///
/// If you ever find yourself manually implementing [SomeIp] you might want to
/// include a test, that calls [VerifySomeIpType::verify] on the type you created.
pub trait VerifySomeIpType {
    /// Verify the type and panic if any errors are found.
    /// # Panics
    /// Panics if the type is not valid.
    /// # Examples
    /// ```
    /// # use serde_someip::types::*;
    /// let e = SomeIpString{min_size: 0, max_size: 42, length_field_size: None};
    /// e.verify();
    /// ```
    ///
    /// ```should_panic
    /// # use serde_someip::types::*;
    /// let e = SomeIpString{min_size: 43, max_size: 42, length_field_size: None};
    /// e.verify(); //panic here
    /// ```
    fn verify(&self);
}

impl VerifySomeIpType for SomeIpEnum {
    fn verify(&self) {
        if matches!(
            self.raw_type,
            SomeIpPrimitive::Bool | SomeIpPrimitive::F32 | SomeIpPrimitive::F64
        ) {
            panic!(
                "Enums cannot use bool, f32 or f64 as raw type, was={}",
                self.raw_type
            );
        }
        self.values.iter().for_each(|(_, value)| {
            if !value.matches(&self.raw_type) {
                panic!(
                    "All values of an enum must be of same type, expected={} was={}",
                    self.raw_type,
                    value.display_type()
                );
            }
        });
    }
}

impl VerifySomeIpType for SomeIpField {
    fn verify(&self) {
        if let Some(id) = self.id {
            if id > 0xFFF {
                panic!("Field ids must not be larger than 0xFFF, was {:X}", id);
            }
        }
        self.field_type.verify();
    }
}

impl VerifySomeIpType for SomeIpString {
    fn verify(&self) {
        if self.max_size < self.min_size {
            panic!(
                "max_size must be bigger or equal to min_size, was max=size={}, min_size={}",
                self.max_size, self.min_size
            );
        }
        if self.max_size > u32::max_value() as usize {
            panic!("SomeIp only supports string upto 4GB in length");
        }
    }
}

impl VerifySomeIpType for SomeIpSequence {
    fn verify(&self) {
        if self.max_elements < self.min_elements {
            panic!("max_elements must be bigger or equal to min_elements, was max_elements={}, min_elements={}", 
            self.max_elements, self.min_elements);
        }
        if self.max_elements > u32::max_value() as usize {
            panic!("SomeIp only supports sequences with upto 2^32 elements");
        }
        self.element_type.verify();
    }
}

impl VerifySomeIpType for SomeIpStruct {
    fn verify(&self) {
        self.fields.iter().for_each(|field| {
            if self.uses_tlv_serialization {
                if field.id.is_none() {
                    panic!(
                        "All fields in a tlv struct must have an id {} has none",
                        field.name
                    );
                }
            } else if field.id.is_some() {
                panic!(
                    "No field in a non tlv struct must have an id, {} has some",
                    field.name
                );
            }
            field.verify();
        });
    }
}

impl VerifySomeIpType for SomeIpType {
    fn verify(&self) {
        match self {
            SomeIpType::Primitive(_) => {}
            SomeIpType::Enum(e) => e.verify(),
            SomeIpType::String(s) => s.verify(),
            SomeIpType::Sequence(s) => s.verify(),
            SomeIpType::Struct(s) => s.verify(),
        }
    }
}

/// A trait to get the SomeIpType info from a rust type.
/// You should usually not implement this yourself but use `#[derive(SomeIp)]` on the struct or enum instead.
pub trait SomeIp {
    /// The SomeIpType data associated with the type.
    const SOMEIP_TYPE: SomeIpType;
}

macro_rules! declare_primitive {
    ($ty:ty = $enum:path) => {
        impl SomeIp for $ty {
            const SOMEIP_TYPE: SomeIpType = SomeIpType::Primitive($enum);
        }
    };
}

declare_primitive!(bool = SomeIpPrimitive::Bool);
declare_primitive!(u8 = SomeIpPrimitive::U8);
declare_primitive!(u16 = SomeIpPrimitive::U16);
declare_primitive!(u32 = SomeIpPrimitive::U32);
declare_primitive!(u64 = SomeIpPrimitive::U64);
declare_primitive!(i8 = SomeIpPrimitive::I8);
declare_primitive!(i16 = SomeIpPrimitive::I16);
declare_primitive!(i32 = SomeIpPrimitive::I32);
declare_primitive!(i64 = SomeIpPrimitive::I64);
declare_primitive!(f32 = SomeIpPrimitive::F32);
declare_primitive!(f64 = SomeIpPrimitive::F64);
