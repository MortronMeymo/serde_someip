//! This module contains the serializer and provides functions to serialize someip encoded data.

use super::error::{Error, Result};
use super::length_fields::LengthFieldSize;
use super::options::*;
use super::types::*;
use super::wire_type::WireType;

use serde::ser::*;

use std::marker::PhantomData;

trait SomeIpWriter {
    fn write(&mut self, data: &[u8]) -> Result<()>;

    fn copy_within<R: std::ops::RangeBounds<usize>>(&mut self, src: R, dest: usize);

    fn len(&self) -> usize;

    unsafe fn set_len(&mut self, len: usize);

    #[inline]
    fn put_zeros(&mut self, len: usize) -> Result<()> {
        assert!(len <= 4);
        let array = [0; 4];
        self.write(&array[..len])
    }

    #[inline]
    fn write_u8(&mut self, data: u8) -> Result<()> {
        let array = [data];
        self.write(&array)
    }

    #[inline]
    fn write_ux<E, T>(&mut self, data: T, byte_order: ByteOrder) -> Result<()>
    where
        E: std::fmt::Debug,
        T: TryInto<u8, Error = E>
            + From<u8>
            + std::ops::BitAnd<Output = T>
            + std::ops::Shr<usize, Output = T>
            + Copy,
    {
        let size = std::mem::size_of::<T>();
        assert!(size <= 8);
        let mut array = [0u8; 8];
        match byte_order {
            ByteOrder::BigEndian => {
                //to me it seams more intutive that we iterate over the size of the primitive type
                //rather than the elements of the array, but clippy disagrees...
                #[allow(clippy::needless_range_loop)]
                for i in 0..size {
                    array[i] = ((data >> (8 * (size - i - 1))) & 0xFFu8.into())
                        .try_into()
                        .unwrap();
                }
            }
            ByteOrder::LittleEndian =>
            {
                #[allow(clippy::needless_range_loop)]
                for i in 0..size {
                    array[i] = ((data >> (8 * i)) & 0xFFu8.into()).try_into().unwrap();
                }
            }
        }
        self.write(&array[..size])
    }
}

impl SomeIpWriter for Vec<u8> {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.extend_from_slice(data);
        Ok(())
    }

    #[inline]
    fn copy_within<R: std::ops::RangeBounds<usize>>(&mut self, src: R, dest: usize) {
        self.as_mut_slice().copy_within(src, dest);
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline]
    unsafe fn set_len(&mut self, len: usize) {
        self.set_len(len);
    }
}

struct Phony;
impl SerializeTuple for Phony {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl SerializeTupleStruct for Phony {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl SerializeTupleVariant for Phony {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl SerializeMap for Phony {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, _key: &T) -> Result<()> {
        unimplemented!()
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}

impl SerializeStructVariant for Phony {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<()> {
        unimplemented!()
    }

    fn end(self) -> Result<()> {
        unimplemented!()
    }
}
struct SomeIpSeqSerializer<'a, Options: SomeIpOptions, Writer: SomeIpWriter> {
    serializer: &'a mut SomeIpSerializer<Options, Writer>,
    someip_type: &'static SomeIpSequence,
    length_field_size: Option<LengthFieldSize>,
    element_count: usize,
}

impl<'a, Options: SomeIpOptions, Writer: SomeIpWriter> SomeIpSeqSerializer<'a, Options, Writer> {
    fn new(
        serializer: &'a mut SomeIpSerializer<Options, Writer>,
        len: Option<usize>,
    ) -> Result<Self> {
        if let SomeIpType::Sequence(s) = serializer.next_type {
            if let Some(len) = len {
                if len < s.min_elements {
                    return Err(Error::NotEnoughData {
                        min: s.min_elements,
                        actual: len,
                    });
                } else if len > s.max_elements {
                    return Err(Error::TooMuchData {
                        max: s.max_elements,
                        actual: len,
                    });
                }
            }

            let length_field_size =
                s.wanted_length_field::<Options>(serializer.is_in_tlv_struct)?;

            Ok(SomeIpSeqSerializer {
                serializer,
                someip_type: s,
                length_field_size,
                element_count: 0,
            })
        } else {
            panic!("Expeceted a sequence but found {}", serializer.next_type)
        }
    }

    fn start(&mut self) -> Result<()> {
        if let Some(configured) = self.length_field_size {
            self.serializer.begin_length_delimited_section(
                configured,
                LengthFieldSize::minimum_length_for(
                    self.someip_type
                        .max_len::<Options>(self.serializer.is_in_tlv_struct)?,
                ),
            )?;
        }
        Ok(())
    }
}

impl<'a, Options: SomeIpOptions, Writer: SomeIpWriter> SerializeSeq
    for SomeIpSeqSerializer<'a, Options, Writer>
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.element_count += 1;
        self.serializer.next_type = self.someip_type.element_type;
        self.serializer.is_in_tlv_struct = false;
        value.serialize(&mut *self.serializer)
    }

    fn end(self) -> Result<()> {
        if self.element_count < self.someip_type.min_elements {
            return Err(Error::NotEnoughData {
                min: self.someip_type.min_elements,
                actual: self.element_count,
            });
        } else if self.element_count > self.someip_type.max_elements {
            return Err(Error::TooMuchData {
                max: self.someip_type.max_elements,
                actual: self.element_count,
            });
        }
        if let Some(s) = self.length_field_size {
            self.serializer.end_length_delimited_section(s)?;
        }
        Ok(())
    }
}

struct SomeIpStructSerializer<'a, Options: SomeIpOptions, Writer: SomeIpWriter> {
    serializer: &'a mut SomeIpSerializer<Options, Writer>,
    struct_type: &'static SomeIpStruct,
    length_field_size: Option<LengthFieldSize>,
}

impl<'a, Options: SomeIpOptions, Writer: SomeIpWriter> SomeIpStructSerializer<'a, Options, Writer> {
    fn new(serializer: &'a mut SomeIpSerializer<Options, Writer>, len: usize) -> Result<Self> {
        if let SomeIpType::Struct(s) = serializer.next_type {
            if len != s.field_count() {
                panic!("Cannot serialize more fields than struct {} has", s.name);
            }
            let length_field_size =
                s.wanted_length_field::<Options>(serializer.is_in_tlv_struct)?;
            Ok(SomeIpStructSerializer {
                serializer,
                struct_type: s,
                length_field_size,
            })
        } else {
            panic!("Expeceted a struct but found {}", serializer.next_type)
        }
    }

    fn start(&mut self) -> Result<()> {
        if let Some(configured) = self.length_field_size {
            self.serializer.begin_length_delimited_section(
                configured,
                LengthFieldSize::minimum_length_for(
                    self.struct_type
                        .max_len::<Options>(self.serializer.is_in_tlv_struct)?,
                ),
            )?;
        }
        Ok(())
    }
}

impl<'a, Options: SomeIpOptions, Writer: SomeIpWriter> SerializeStruct
    for SomeIpStructSerializer<'a, Options, Writer>
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        let field = self.struct_type.field_by_name(key).unwrap_or_else(|| {
            panic!(
                "Cannot find field {} in struct {}",
                key, self.struct_type.name
            )
        });

        if self.struct_type.uses_tlv() {
            if field.id.is_none() {
                panic!(
                    "Field {} in struct {} has no id despite the struct using tlv",
                    key, self.struct_type.name
                );
            }

            let wire_type = field.field_type.get_wire_type();
            let tag = u16::from(wire_type) | field.id.unwrap();
            let tag_pos = self.serializer.writer.len();
            tag.serialize(&mut *self.serializer)?;

            self.serializer.is_in_tlv_struct = self.struct_type.uses_tlv();
            self.serializer.next_type = field.field_type;
            value.serialize(&mut *self.serializer)?;
            if wire_type == WireType::LengthDelimitedFromConfig {
                let (actual, is_as_configured) = self.serializer.last_length_field.take().unwrap();
                if !is_as_configured || !Options::SERIALIZER_USE_LEGACY_WIRE_TYPE {
                    let tag = u16::from(WireType::from(actual)) | field.id.unwrap();
                    self.serializer.write_at_previous_pos(tag_pos, tag)?;
                }
            }
            Ok(())
        } else {
            self.serializer.is_in_tlv_struct = self.struct_type.uses_tlv();
            self.serializer.next_type = field.field_type;
            value.serialize(&mut *self.serializer)
        }
    }

    fn end(self) -> Result<()> {
        if let Some(s) = self.length_field_size {
            self.serializer.end_length_delimited_section(s)?;
        }
        self.serializer.is_in_tlv_struct = false;
        Ok(())
    }
}

struct SomeIpSerializer<Options: SomeIpOptions, Writer: SomeIpWriter> {
    writer: Writer,
    next_type: &'static SomeIpType,
    is_in_tlv_struct: bool,
    last_length_field: Option<(LengthFieldSize, bool)>,
    length_delimited_sections: Vec<(usize, LengthFieldSize, bool)>,
    phantom: PhantomData<Options>,
}

impl<Options: SomeIpOptions, Writer: SomeIpWriter> SomeIpSerializer<Options, Writer> {
    fn new(writer: Writer, root_type: &'static SomeIpType) -> SomeIpSerializer<Options, Writer> {
        SomeIpSerializer {
            writer,
            next_type: root_type,
            is_in_tlv_struct: false,
            last_length_field: None,
            length_delimited_sections: Vec::default(),
            phantom: PhantomData,
        }
    }

    fn finish(self) -> Writer {
        self.writer
    }

    fn begin_length_delimited_section(
        &mut self,
        configured: LengthFieldSize,
        maximum_needed: LengthFieldSize,
    ) -> Result<()> {
        let pos = self.writer.len();
        let reserved = std::cmp::max(configured, maximum_needed);
        self.length_delimited_sections
            .push((pos, reserved, self.is_in_tlv_struct));
        let size = reserved.into();
        self.writer.put_zeros(size)
    }

    fn end_length_delimited_section(&mut self, configured: LengthFieldSize) -> Result<()> {
        let (pos, reserved, was_in_tlv) = self.length_delimited_sections.pop().unwrap();
        let end = self.writer.len();
        let len = end - pos - usize::from(reserved);

        let actual = Options::select_length_field_size(configured, len, was_in_tlv)?;
        self.last_length_field = Some((actual, actual == configured));

        if actual != reserved {
            assert!(reserved > actual);
            self.writer
                .copy_within(pos + usize::from(reserved)..end, pos + usize::from(actual));
            let difference = usize::from(reserved) - usize::from(actual);
            unsafe {
                //safe we just trim the buffer to size
                self.writer.set_len(end - difference);
            }
        }

        match actual {
            LengthFieldSize::OneByte => {
                if len > u8::max_value() as usize {
                    Err(Error::TooLong {
                        actual_length: len,
                        length_field_size: actual,
                    })
                } else {
                    self.write_at_previous_pos(pos, len as u8)
                }
            }
            LengthFieldSize::TwoBytes => {
                if len > u16::max_value() as usize {
                    Err(Error::TooLong {
                        actual_length: len,
                        length_field_size: actual,
                    })
                } else {
                    self.write_at_previous_pos(pos, len as u16)
                }
            }
            LengthFieldSize::FourBytes => {
                if len > u32::max_value() as usize {
                    Err(Error::TooLong {
                        actual_length: len,
                        length_field_size: actual,
                    })
                } else {
                    self.write_at_previous_pos(pos, len as u32)
                }
            }
        }
    }

    fn write_at_previous_pos<T: Serialize>(&mut self, pos: usize, len: T) -> Result<()> {
        let end = self.writer.len();
        assert!(pos < end);
        unsafe {
            //safe because this shortens the buffer
            self.writer.set_len(pos);
        }
        len.serialize(&mut *self)?;
        unsafe {
            //safe because we just restore the original len
            self.writer.set_len(end);
        }
        Ok(())
    }

    fn internal_write_str(&mut self, v: &str) -> Result<()> {
        if Options::STRING_ENCODING != StringEncoding::Utf16 {
            self.writer.write(v.as_bytes())
        } else {
            v.encode_utf16().try_for_each(|c| c.serialize(&mut *self))
        }
    }
}

impl<'a, Options: SomeIpOptions, Writer: SomeIpWriter> Serializer
    for &'a mut SomeIpSerializer<Options, Writer>
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SomeIpSeqSerializer<'a, Options, Writer>;
    type SerializeTuple = Phony;
    type SerializeTupleStruct = Phony;
    type SerializeTupleVariant = Phony;
    type SerializeMap = Phony;
    type SerializeStruct = SomeIpStructSerializer<'a, Options, Writer>;
    type SerializeStructVariant = Phony;

    fn serialize_bool(self, v: bool) -> Result<()> {
        if v {
            self.writer.write_u8(1)
        } else {
            self.writer.write_u8(0)
        }
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.writer.write_u8(v as u8)
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.writer.write_ux(v as u16, Options::BYTE_ORDER)
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.writer.write_ux(v as u32, Options::BYTE_ORDER)
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.writer.write_ux(v as u64, Options::BYTE_ORDER)
    }

    fn serialize_i128(self, _v: i128) -> Result<()> {
        unimplemented!("i128 is not supported by someip")
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.writer.write_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.writer.write_ux(v, Options::BYTE_ORDER)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.writer.write_ux(v, Options::BYTE_ORDER)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.writer.write_ux(v, Options::BYTE_ORDER)
    }

    fn serialize_u128(self, _v: u128) -> Result<()> {
        unimplemented!("u128 is not supported by someip")
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.writer.write_ux(v.to_bits(), Options::BYTE_ORDER)
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.writer.write_ux(v.to_bits(), Options::BYTE_ORDER)
    }

    fn serialize_char(self, _v: char) -> Result<()> {
        unimplemented!("char is not supported by someip")
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        if let SomeIpType::String(s) = self.next_type {
            if Options::STRING_ENCODING == StringEncoding::Ascii && !v.is_ascii() {
                return Err(Error::CannotCodeString(
                    "Encoding is ASCII but str contains non ASCII chars".into(),
                ));
            }

            let length_field_size = s.wanted_length_field::<Options>(self.is_in_tlv_struct)?;

            if let Some(configured) = length_field_size {
                self.begin_length_delimited_section(
                    configured,
                    LengthFieldSize::minimum_length_for(
                        s.max_len::<Options>(self.is_in_tlv_struct)?,
                    ),
                )?;
            }

            let begin = self.writer.len();
            if Options::STRING_WITH_BOM {
                self.internal_write_str("\u{FEFF}")?;
            }
            self.internal_write_str(v)?;
            if Options::STRING_WITH_TERMINATOR {
                self.internal_write_str("\0")?;
            }
            let actual_len = self.writer.len() - begin;

            if actual_len < s.min_size {
                return Err(Error::NotEnoughData {
                    min: s.min_size,
                    actual: actual_len,
                });
            } else if actual_len > s.max_size {
                return Err(Error::TooMuchData {
                    max: s.max_size,
                    actual: actual_len,
                });
            }

            if let Some(s) = length_field_size {
                self.end_length_delimited_section(s)?;
            }

            Ok(())
        } else {
            panic!("Expeceted a string but found {}", self.next_type)
        }
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        if let SomeIpType::Sequence(s) = self.next_type {
            if !matches!(s.element_type, SomeIpType::Primitive(SomeIpPrimitive::U8)) {
                panic!("Expeceted Primitive(u8) bu found {}", s.element_type)
            }
            if v.len() < s.min_elements {
                return Err(Error::NotEnoughData {
                    min: s.min_elements,
                    actual: v.len(),
                });
            } else if v.len() > s.max_elements {
                return Err(Error::TooMuchData {
                    max: s.max_elements,
                    actual: v.len(),
                });
            }

            let length_field_size = s.wanted_length_field::<Options>(self.is_in_tlv_struct)?;

            if let Some(configured) = length_field_size {
                self.begin_length_delimited_section(
                    configured,
                    LengthFieldSize::minimum_length_for(
                        s.max_len::<Options>(self.is_in_tlv_struct)?,
                    ),
                )?;
            }
            self.writer.write(v)?;
            if let Some(s) = length_field_size {
                self.end_length_delimited_section(s)?;
            }
            Ok(())
        } else {
            panic!("Expeceted a sequence of u8s but found {}", self.next_type)
        }
    }

    fn serialize_none(self) -> Result<()> {
        if !self.is_in_tlv_struct {
            panic!("Options are only supported in tlv structs");
        }
        unsafe {
            //remove tag, safe because a tag was written for exactly 2 bytes
            self.writer.set_len(self.writer.len() - 2);
        }
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<()> {
        if !self.is_in_tlv_struct {
            panic!("Options are only supported in tlv structs");
        }
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        let value = if let SomeIpType::Enum(e) = self.next_type {
            e.name_to_value(variant)
                .unwrap_or_else(|| panic!("Enum {} has no field {}", e.name, variant))
        } else {
            panic!("Expeceted an enum but found {}", self.next_type)
        };

        match value {
            SomeIpEnumValue::U8(v) => v.serialize(self),
            SomeIpEnumValue::U16(v) => v.serialize(self),
            SomeIpEnumValue::U32(v) => v.serialize(self),
            SomeIpEnumValue::U64(v) => v.serialize(self),
            SomeIpEnumValue::I8(v) => v.serialize(self),
            SomeIpEnumValue::I16(v) => v.serialize(self),
            SomeIpEnumValue::I32(v) => v.serialize(self),
            SomeIpEnumValue::I64(v) => v.serialize(self),
        }
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        unimplemented!(
            "Newtype variants (e.g.: enum Foo {{ Bar(u32) }} ) are not supported by someip"
        )
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        let mut seq_serializer = SomeIpSeqSerializer::new(self, len)?;
        seq_serializer.start()?;
        Ok(seq_serializer)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Phony> {
        unimplemented!("Tuples (e.g.: (u32, i64,...) ) are not supported by someip")
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Phony> {
        unimplemented!(
            "Tuple structs (e.g.: struct Foor(u32, i64, ...) ) are not supported by someip"
        )
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Phony> {
        unimplemented!(
            "Tuple variants (e.g.: enum Foo {{ Bar(u32, i64, ...) }} ) are not supported by someip"
        )
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Phony> {
        unimplemented!("Maps are not supported by someip")
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        let mut struct_serializer = SomeIpStructSerializer::new(self, len)?;
        struct_serializer.start()?;
        Ok(struct_serializer)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Phony> {
        unimplemented!("Struct variants (e.g.: enum Foo {{ Bar{{ a: u32, b: i64, ...}} }} ) are not supported by someip")
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[inline]
fn to_vec_manuel<Options, T>(value: &T, someip_type: &'static SomeIpType) -> Result<Vec<u8>>
where
    Options: SomeIpOptions,
    T: Serialize,
{
    #[cfg(debug_assertions)]
    {
        Options::verify_string_encoding();
        someip_type.verify();
    }
    let mut serializer = SomeIpSerializer::<Options, Vec<u8>>::new(Vec::default(), someip_type);
    value.serialize(&mut serializer)?;
    Ok(serializer.finish())
}

/// Serialises the value to a [Vec<u8>]
///
/// # Panics
/// This function panics if the implementation of the [SomeIp](super::SomeIp) trait
/// produces invalid type information or this information is incompatible with the [Serialize](serde::Serialize) implementation.
pub fn to_vec<Options, T>(value: &T) -> Result<Vec<u8>>
where
    Options: SomeIpOptions,
    T: Serialize + SomeIp,
{
    to_vec_manuel::<Options, _>(value, &T::SOMEIP_TYPE)
}

#[test]
fn test_bool() {
    assert_eq!(vec![0], to_vec::<ExampleOptions, _>(&false).unwrap());
    assert_eq!(vec![1], to_vec::<ExampleOptions, _>(&true).unwrap());
}

#[test]
fn test_u8() {
    assert_eq!(vec![42], to_vec::<ExampleOptions, _>(&42u8).unwrap());
    assert_eq!(vec![0], to_vec::<ExampleOptions, _>(&0u8).unwrap());
    assert_eq!(
        vec![255],
        to_vec::<ExampleOptions, _>(&u8::max_value()).unwrap()
    );
}

#[test]
fn test_i8() {
    assert_eq!(vec![42], to_vec::<ExampleOptions, _>(&42i8).unwrap());
    assert_eq!(vec![0], to_vec::<ExampleOptions, _>(&0i8).unwrap());
    assert_eq!(vec![0xFF], to_vec::<ExampleOptions, _>(&-1i8).unwrap());
    assert_eq!(
        vec![0x7F],
        to_vec::<ExampleOptions, _>(&i8::max_value()).unwrap()
    );
    assert_eq!(
        vec![0x80],
        to_vec::<ExampleOptions, _>(&i8::min_value()).unwrap()
    );
}

#[cfg(test)]
macro_rules! test_endianess {
    ($test_name:ident, $($to_test:expr => $expected:expr),+) => {
        #[test]
        fn $test_name() {
            $(
                {
                    let mut expected = $expected;
                    assert_eq!(expected, to_vec::<ExampleOptions,_>(&$to_test).unwrap());
                    expected.reverse();
                    assert_eq!(expected, to_vec::<test::LittleEndianOptions,_>(&$to_test).unwrap());
                }
            )*
        }
    };
}

#[cfg(test)]
test_endianess!(test_u16,
    42u16 => vec![0, 42],
    0u16 => vec![0, 0],
    u16::max_value() => vec![0xFF, 0xFF]);

#[cfg(test)]
test_endianess!(test_i16,
    42i16 => vec![0, 42],
    0i16 => vec![0, 0],
    i16::max_value() => vec![0x7F, 0xFF],
    -1i16 => vec![0xFF, 0xFF],
    i16::min_value() => vec![0x80, 0]);

#[cfg(test)]
test_endianess!(test_u32,
    42u32 => vec![0, 0, 0, 42],
    0u32 => vec![0; 4],
    u32::max_value() => vec![0xFF; 4]);

#[cfg(test)]
test_endianess!(test_i32,
    42i32 => vec![0, 0, 0, 42],
    0i32 => vec![0; 4],
    i32::max_value() => vec![0x7F, 0xFF, 0xFF, 0xFF],
    -1i32 => vec![0xFF; 4],
    i32::min_value() => vec![0x80, 0, 0, 0]);

#[cfg(test)]
test_endianess!(test_u64,
    42u64 => vec![0, 0, 0, 0, 0, 0, 0, 42],
    0u64 => vec![0; 8],
    u64::max_value() => vec![0xFF; 8]);

#[cfg(test)]
test_endianess!(test_i64,
    42i64 => vec![0, 0, 0, 0, 0, 0, 0, 42],
    0i64 => vec![0; 8],
    i64::max_value() => vec![0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    -1i64 => vec![0xFF; 8],
    i64::min_value() => vec![0x80, 0, 0, 0, 0, 0, 0, 0]);

#[cfg(test)]
test_endianess!(test_f32,
    0f32 => vec![0; 4],
    1f32 => vec![0x3F, 0x80, 0, 0],
    -1f32 => vec![0xBF, 0x80, 0, 0],
    0.5f32 => vec![0x3F, 0, 0, 0],
    -0.5f32 => vec![0xBF, 0, 0, 0],
    f32::INFINITY => vec![0x7F, 0x80, 0, 0],
    f32::NEG_INFINITY => vec![0xFF, 0x80, 0, 0],
    f32::MAX => vec![0x7F, 0x7F, 0xFF, 0xFF],
    f32::MIN => vec![0xFF, 0x7F, 0xFF, 0xFF]);

#[cfg(test)]
test_endianess!(test_f64,
    0f64 => vec![0; 8],
    1f64 => vec![0x3F, 0xF0, 0, 0, 0, 0, 0, 0],
    -1f64 => vec![0xBF, 0xF0, 0, 0, 0, 0, 0, 0],
    0.5f64 => vec![0x3F, 0xE0, 0, 0, 0, 0, 0, 0],
    -0.5f64 => vec![0xBF, 0xE0, 0, 0, 0, 0, 0, 0],
    f64::INFINITY => vec![0x7F, 0xF0, 0, 0, 0, 0, 0, 0],
    f64::NEG_INFINITY => vec![0xFF, 0xF0, 0, 0, 0, 0, 0, 0],
    f64::MAX => vec![0x7F, 0xEF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    f64::MIN => vec![0xFF, 0xEF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

#[test]
fn test_enum() {
    #[derive(Debug, serde::Serialize)]
    enum TestEnum {
        First,
        Second,
        Third,
    }

    impl SomeIp for TestEnum {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Enum(SomeIpEnum {
            name: "TestEnum",
            raw_type: SomeIpPrimitive::I16,
            values: &[
                ("First", SomeIpEnumValue::I16(0)),
                ("Second", SomeIpEnumValue::I16(42)),
                ("Third", SomeIpEnumValue::I16(-1337)),
            ],
        });
    }

    let result = to_vec::<ExampleOptions, _>(&TestEnum::First).unwrap();
    assert_eq!(vec![0, 0], result);

    let result = to_vec::<ExampleOptions, _>(&TestEnum::Second).unwrap();
    assert_eq!(vec![0, 42], result);

    let result = to_vec::<ExampleOptions, _>(&TestEnum::Third).unwrap();
    assert_eq!(vec![0xFA, 0xC7], result);
}

#[test]
fn test_bytes() {
    use bytes::{BufMut, BytesMut};
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 32,
        min_elements: 0,
        element_type: &u8::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });
    let mut bytes = BytesMut::with_capacity(32);

    bytes.put_u32(0x12345678);
    assert_eq!(
        vec![0, 4, 0x12, 0x34, 0x56, 0x78],
        to_vec_manuel::<ExampleOptions, _>(&bytes, &SOMEIP_TYPE).unwrap()
    );

    bytes.put_u32(0x90ABCDEF);
    assert_eq!(
        vec![0, 8, 0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF],
        to_vec_manuel::<ExampleOptions, _>(&bytes, &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_bytes_no_length_field() {
    use bytes::{BufMut, BytesMut};
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 4,
        min_elements: 4,
        element_type: &u8::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });
    let mut bytes = BytesMut::with_capacity(4);
    bytes.put_u32(0x12345678);
    assert_eq!(
        vec![0x12, 0x34, 0x56, 0x78],
        to_vec_manuel::<ExampleOptions, _>(&bytes, &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_i16_seq() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 4,
        min_elements: 0,
        element_type: &i16::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });

    let values = vec![42i16, -5];
    assert_eq!(
        vec![0, 4, 0, 42, 0xFF, 0xFB],
        to_vec_manuel::<ExampleOptions, _>(&values, &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_i16_seq_no_length_field() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 4,
        min_elements: 4,
        element_type: &i16::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });

    let values = vec![42i16, -5, 1, 1337];
    assert_eq!(
        vec![0, 42, 0xFF, 0xFB, 0, 1, 0x5, 0x39],
        to_vec_manuel::<ExampleOptions, _>(&values, &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_2_dimensional_seq() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 4,
        min_elements: 0,
        element_type: &SomeIpType::Sequence(SomeIpSequence {
            max_elements: 3,
            min_elements: 0,
            element_type: &i8::SOMEIP_TYPE,
            length_field_size: Some(LengthFieldSize::OneByte),
        }),
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });

    let values = vec![vec![1i8, -2, 3], vec![42], vec![]];
    assert_eq!(
        vec![0, 7, 3, 1, 0xFE, 3, 1, 42, 0],
        to_vec_manuel::<ExampleOptions, _>(&values, &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 4,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });

    assert_eq!(
        vec![2, 0x68, 0x69],
        to_vec_manuel::<ExampleOptions, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_no_length_field() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 2,
        min_size: 2,
        length_field_size: Some(LengthFieldSize::OneByte),
    });

    assert_eq!(
        vec![0x68, 0x69],
        to_vec_manuel::<ExampleOptions, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf8_bom() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 5,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_BOM: bool = true;
    }

    assert_eq!(
        vec![5, 0xEF, 0xBB, 0xBF, 0x68, 0x69],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf8_terminator() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 5,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_TERMINATOR: bool = true;
    }

    assert_eq!(
        vec![3, 0x68, 0x69, 0],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf16() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 4,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });

    struct Options;
    impl SomeIpOptions for Options {
        const STRING_ENCODING: StringEncoding = StringEncoding::Utf16;
    }

    assert_eq!(
        vec![4, 0, 0x68, 0, 0x69],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf16_bom() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 6,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_ENCODING: StringEncoding = StringEncoding::Utf16;
        const STRING_WITH_BOM: bool = true;
    }

    assert_eq!(
        vec![6, 0xFE, 0xFF, 0, 0x68, 0, 0x69],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf16_bom_le() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 6,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    struct Options;
    impl SomeIpOptions for Options {
        const BYTE_ORDER: ByteOrder = ByteOrder::LittleEndian;
        const STRING_ENCODING: StringEncoding = StringEncoding::Utf16;
        const STRING_WITH_BOM: bool = true;
    }

    assert_eq!(
        vec![6, 0xFF, 0xFE, 0x68, 0, 0x69, 0],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_utf16_terminator() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        max_size: 6,
        min_size: 0,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_ENCODING: StringEncoding = StringEncoding::Utf16;
        const STRING_WITH_TERMINATOR: bool = true;
    }

    assert_eq!(
        vec![6, 0, 0x68, 0, 0x69, 0, 0],
        to_vec_manuel::<Options, _>(&"hi", &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_struct() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        first: i16,
        second: u32,
        third: Vec<u8>,
    }
    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[
                SomeIpField {
                    name: "first",
                    id: None,
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "second",
                    id: None,
                    field_type: &u32::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "third",
                    id: None,
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        max_elements: 5,
                        min_elements: 1,
                        element_type: &u8::SOMEIP_TYPE,
                        length_field_size: Some(LengthFieldSize::OneByte),
                    }),
                },
            ],
            uses_tlv_serialization: false,
            is_message_wrapper: false,
            length_field_size: None,
        });
    }

    let teste = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };

    assert_eq!(
        vec![0xFF, 0xFF, 0, 0, 0, 42, 3, 1, 2, 3],
        to_vec::<ExampleOptions, _>(&teste).unwrap()
    );
}

#[test]
fn test_struct_beginning_length_field() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        first: i16,
        second: u32,
        third: Vec<u8>,
    }
    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[
                SomeIpField {
                    name: "first",
                    id: None,
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "second",
                    id: None,
                    field_type: &u32::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "third",
                    id: None,
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        max_elements: 5,
                        min_elements: 1,
                        element_type: &u8::SOMEIP_TYPE,
                        length_field_size: Some(LengthFieldSize::OneByte),
                    }),
                },
            ],
            uses_tlv_serialization: false,
            is_message_wrapper: false,
            length_field_size: Some(LengthFieldSize::TwoBytes),
        });
    }

    let teste = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        vec![0, 10, 0xFF, 0xFF, 0, 0, 0, 42, 3, 1, 2, 3],
        to_vec::<ExampleOptions, _>(&teste).unwrap()
    );
}

#[test]
fn test_struct_tlv() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        first: i16,
        second: u32,
        third: Vec<u8>,
    }

    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[
                SomeIpField {
                    name: "first",
                    id: Some(1),
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "second",
                    id: Some(2),
                    field_type: &u32::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "third",
                    id: Some(3),
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        max_elements: 5,
                        min_elements: 1,
                        element_type: &u8::SOMEIP_TYPE,
                        length_field_size: Some(LengthFieldSize::OneByte),
                    }),
                },
            ],
            uses_tlv_serialization: true,
            is_message_wrapper: false,
            length_field_size: Some(LengthFieldSize::TwoBytes),
        });
    }

    let teste = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        vec![0, 16, 0x10, 0x01, 0xFF, 0xFF, 0x20, 0x02, 0, 0, 0, 42, 0x50, 0x03, 3, 1, 2, 3],
        to_vec::<ExampleOptions, _>(&teste).unwrap()
    );
}

#[test]
fn test_struct_tlv_message_wrapper() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        first: i16,
        second: u32,
        third: Vec<u8>,
    }
    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[
                SomeIpField {
                    name: "first",
                    id: Some(1),
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "second",
                    id: Some(2),
                    field_type: &u32::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "third",
                    id: Some(3),
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        max_elements: 5,
                        min_elements: 1,
                        element_type: &u8::SOMEIP_TYPE,
                        length_field_size: Some(LengthFieldSize::OneByte),
                    }),
                },
            ],
            uses_tlv_serialization: true,
            is_message_wrapper: true,
            length_field_size: Some(LengthFieldSize::TwoBytes),
        });
    }

    let teste = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        vec![0x10, 0x01, 0xFF, 0xFF, 0x20, 0x02, 0, 0, 0, 42, 0x50, 0x03, 3, 1, 2, 3],
        to_vec::<ExampleOptions, _>(&teste).unwrap()
    );
}

#[test]
fn test_uses_smallest_length_field_size() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        a: Vec<u8>,
    }
    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[SomeIpField {
                name: "a",
                id: Some(0),
                field_type: &SomeIpType::Sequence(SomeIpSequence {
                    max_elements: 1 << 24,
                    min_elements: 0,
                    element_type: &u8::SOMEIP_TYPE,
                    length_field_size: Some(LengthFieldSize::FourBytes),
                }),
            }],
            uses_tlv_serialization: true,
            is_message_wrapper: false,
            length_field_size: Some(LengthFieldSize::FourBytes),
        });
    }

    let teste = Test {
        a: vec![1, 2, 3, 4],
    };
    assert_eq!(
        vec![0, 0, 0, 7, 0x50, 0x00, 4, 1, 2, 3, 4],
        to_vec::<ExampleOptions, _>(&teste).unwrap()
    );
}

#[test]
fn test_optional() {
    #[derive(Debug, serde::Serialize)]
    struct Test {
        a: Option<u32>,
    }
    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            name: "Test",
            fields: &[SomeIpField {
                name: "a",
                id: Some(0),
                field_type: &u32::SOMEIP_TYPE,
            }],
            uses_tlv_serialization: true,
            is_message_wrapper: false,
            length_field_size: Some(LengthFieldSize::OneByte),
        });
    }

    assert_eq!(
        vec![6, 0x20, 0x00, 0, 0, 0, 42],
        to_vec::<ExampleOptions, _>(&Test { a: Some(42) }).unwrap()
    );

    assert_eq!(
        vec![0],
        to_vec::<ExampleOptions, _>(&Test { a: None }).unwrap()
    );
}

#[test]
fn test_newtype() {
    #[derive(Debug, serde::Serialize)]
    struct Test(String);

    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
            min_size: 0,
            max_size: 42,
            length_field_size: Some(LengthFieldSize::OneByte),
        });
    }

    assert_eq!(
        vec![2, 0x68, 0x69],
        to_vec::<ExampleOptions, _>(&Test("hi".into())).unwrap()
    );
}
