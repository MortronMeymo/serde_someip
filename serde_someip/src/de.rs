//! This module contains the deserializer and provides functions to deserialize someip encoded data.

use super::error::*;
use super::length_fields::LengthFieldSize;
use super::options::*;
use super::types::*;
use super::wire_type::WireType;

use serde::de::{
    Deserialize, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

use std::io::Read;
use std::marker::PhantomData;

trait SomeIpReader<'de> {
    const CAN_BORROW: bool;

    fn read<T, F: FnOnce(&[u8]) -> Result<T>>(&mut self, len: usize, func: F) -> Result<T>;

    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>>;

    fn discard(&mut self, len: usize) -> Result<()>;

    fn remaining(&self) -> usize;

    fn read_borrowed(&mut self, len: usize) -> Result<&'de [u8]>;

    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        self.read(1, |s| Ok(s[0]))
    }

    #[inline]
    fn read_ux<
        T: Default + From<u8> + std::ops::BitOr<Output = T> + std::ops::Shl<usize, Output = T>,
    >(
        &mut self,
        byte_order: ByteOrder,
    ) -> Result<T> {
        let parser = match byte_order {
            ByteOrder::BigEndian => |slice: &[u8]| {
                let mut res = T::default();
                for b in slice {
                    res = res << 8 | T::from(*b);
                }
                Ok(res)
            },
            ByteOrder::LittleEndian => |slice: &[u8]| {
                let mut res = T::default();
                for (i, b) in slice.iter().enumerate() {
                    res = res | (T::from(*b) << (i * 8));
                }
                Ok(res)
            },
        };
        self.read(std::mem::size_of::<T>(), parser)
    }
}

#[inline]
fn next_slice<'de>(buf: &mut &'de [u8], len: usize) -> Result<&'de [u8]> {
    if buf.len() < len {
        return Err(Error::TooShort);
    }
    let (result, new_buf) = buf.split_at(len);
    *buf = new_buf;
    Ok(result)
}

impl<'de> SomeIpReader<'de> for &'de [u8] {
    const CAN_BORROW: bool = true;

    #[inline]
    fn read<T, F: FnOnce(&[u8]) -> Result<T>>(&mut self, len: usize, func: F) -> Result<T> {
        func(next_slice(self, len)?)
    }

    #[inline]
    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        Ok(Vec::from(next_slice(self, len)?))
    }

    #[inline]
    fn discard(&mut self, len: usize) -> Result<()> {
        next_slice(self, len)?;
        Ok(())
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn read_borrowed(&mut self, len: usize) -> Result<&'de [u8]> {
        next_slice(self, len)
    }
}

struct ExtendedReader<R: Read> {
    reader: R,
    remaining_bytes: usize,
}

impl<R: Read> ExtendedReader<R> {
    const DISCARD_CHUNK_SIZE: usize = 8192;

    fn new(reader: R, len: usize) -> ExtendedReader<R> {
        ExtendedReader {
            reader,
            remaining_bytes: len,
        }
    }

    #[inline]
    fn updat_remaining(&mut self, len: usize) -> Result<()> {
        if len > self.remaining_bytes {
            return Err(Error::TooShort);
        }
        self.remaining_bytes -= len;
        Ok(())
    }
}

impl<'de, R: Read> SomeIpReader<'de> for ExtendedReader<R> {
    const CAN_BORROW: bool = false;

    #[inline]
    fn read<T, F: FnOnce(&[u8]) -> Result<T>>(&mut self, len: usize, func: F) -> Result<T> {
        assert!(len < 8);
        self.updat_remaining(len)?;
        let mut buf = [0; 8];
        let slice = &mut buf[..len];
        self.reader.read_exact(slice)?;
        func(slice)
    }

    #[inline]
    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        self.updat_remaining(len)?;
        let mut buf = Vec::with_capacity(len);
        self.reader
            .by_ref()
            .take(len as u64)
            .read_to_end(&mut buf)?;
        Ok(buf)
    }

    #[inline]
    fn discard(&mut self, len: usize) -> Result<()> {
        use std::cmp::min;

        self.updat_remaining(len)?;
        let chunk_size = min(len, Self::DISCARD_CHUNK_SIZE);
        let mut buf = vec![0; chunk_size];
        let mut len = len;
        while len != 0 {
            let to_read = min(len, Self::DISCARD_CHUNK_SIZE);
            len -= self.reader.read(&mut buf[..to_read])?;
        }
        Ok(())
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.remaining_bytes
    }

    #[inline]
    fn read_borrowed(&mut self, _len: usize) -> Result<&'de [u8]> {
        unimplemented!("Cannot borrow from std::io::Read");
    }
}

struct SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    reader: Reader,
    next_type: &'static SomeIpType,
    next_length_field_size: Option<LengthFieldSize>,
    next_field_name: &'static str,
    is_in_tlv_struct: bool,
    length_delimited_sections: Vec<usize>,
    phantom: PhantomData<Options>,
    phantom2: PhantomData<&'de str>,
}

impl<'de, Options, Reader> SomeIpReader<'de> for SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    const CAN_BORROW: bool = Reader::CAN_BORROW;

    #[inline]
    fn read<T, F: FnOnce(&[u8]) -> Result<T>>(&mut self, len: usize, func: F) -> Result<T> {
        self.before_read(len)?;
        self.reader.read(len, func)
    }

    #[inline]
    fn read_vec(&mut self, len: usize) -> Result<Vec<u8>> {
        self.before_read(len)?;
        self.reader.read_vec(len)
    }

    #[inline]
    fn discard(&mut self, len: usize) -> Result<()> {
        self.before_read(len)?;
        self.reader.discard(len)
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.length_delimited_sections
            .last()
            .cloned()
            .unwrap_or_else(|| self.reader.remaining())
    }

    #[inline]
    fn read_borrowed(&mut self, len: usize) -> Result<&'de [u8]> {
        self.before_read(len)?;
        self.reader.read_borrowed(len)
    }
}

impl<'de, Options, Reader> SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    fn new(
        reader: Reader,
        root_type: &'static SomeIpType,
    ) -> SomeIpDeserializer<'de, Options, Reader> {
        SomeIpDeserializer {
            reader,
            next_type: root_type,
            next_length_field_size: None,
            next_field_name: "",
            is_in_tlv_struct: false,
            length_delimited_sections: Vec::default(),
            phantom: PhantomData,
            phantom2: PhantomData,
        }
    }

    #[inline]
    fn before_read(&mut self, len: usize) -> Result<()> {
        if self.remaining() < len {
            Err(Error::TooShort)
        } else {
            if let Some(v) = self.length_delimited_sections.last_mut() {
                *v -= len;
            }
            Ok(())
        }
    }

    #[inline]
    fn begin_known_length_delimited_section(&mut self, len: usize) -> Result<usize> {
        self.before_read(len)?;
        self.length_delimited_sections.push(len);
        Ok(len)
    }

    #[inline]
    fn begin_length_delimited_section(
        &mut self,
        length_field_size: LengthFieldSize,
    ) -> Result<usize> {
        let size = if let Some(size) = self.next_length_field_size.take() {
            size
        } else {
            length_field_size
        };

        let len = match size {
            LengthFieldSize::OneByte => self.read_u8()? as usize,
            LengthFieldSize::TwoBytes => self.read_ux::<u16>(Options::BYTE_ORDER)? as usize,
            LengthFieldSize::FourBytes => self.read_ux::<u32>(Options::BYTE_ORDER)? as usize,
        };

        self.begin_known_length_delimited_section(len)
    }

    #[inline]
    fn end_length_delimited_section(&mut self) -> Result<()> {
        if let Some(v) = self.length_delimited_sections.pop() {
            if v != 0 {
                Err(Error::NotAllBytesConsumed(v))
            } else {
                Ok(())
            }
        } else {
            panic!("Ended more length delimited sections than where started")
        }
    }

    fn read_utf8_string(&mut self, len: usize) -> Result<String> {
        let mut len = len;
        if Options::STRING_WITH_BOM {
            let bom = "\u{FEFF}".as_bytes();
            if len < bom.len() {
                return Err(Error::CannotCodeString(
                    "String must begin with BOM and cannot be completely empty".into(),
                ));
            }
            if self.read(bom.len(), |actual| Ok(actual != bom))? {
                return Err(Error::CannotCodeString(
                    "String must begin with a BOM".into(),
                ));
            }
            len -= bom.len();
        }

        let mut value = String::from_utf8(self.read_vec(len)?)?;
        if Options::STRING_WITH_TERMINATOR {
            if value.ends_with('\0') {
                value.pop();
            } else {
                return Err(Error::CannotCodeString(
                    "String must end with 0 terminator".into(),
                ));
            }
        }
        if Options::STRING_ENCODING == StringEncoding::Ascii && !value.is_ascii() {
            return Err(Error::CannotCodeString(
                "String contained non ascii chars".into(),
            ));
        }
        Ok(value)
    }

    fn read_utf16_string(&mut self, len: usize) -> Result<String> {
        if len % 2 != 0 {
            return Err(Error::CannotCodeString(
                "UTF-16 strings must always have an even byte length".into(),
            ));
        }
        let mut len = len / 2;

        let byte_order = if Options::STRING_WITH_BOM {
            if len == 0 {
                return Err(Error::CannotCodeString(
                    "String must begin with BOM and can never be completely empty".into(),
                ));
            }
            len -= 1;
            self.read(2, |slice| {
                if slice == [0xFE, 0xFF] {
                    Ok(ByteOrder::BigEndian)
                } else if slice == [0xFF, 0xFE] {
                    Ok(ByteOrder::LittleEndian)
                } else {
                    Err(Error::CannotCodeString("String must begin with BOM".into()))
                }
            })?
        } else {
            Options::BYTE_ORDER
        };
        let u16_parser = || self.read_ux::<u16>(byte_order);

        //I'm not callint String::from_utf16() since that requires the entire string to be in memory
        //as a [u16] and then copies the data. With the custom iterator below we can immediately construct
        //the string in place from utf16 chars.
        struct CharIter<Parser: FnMut() -> Result<u16>> {
            len: usize,
            parser: Parser,
            count: usize,
            last_result: Result<u16>,
        }
        impl<Parser: FnMut() -> Result<u16>> Iterator for CharIter<Parser> {
            type Item = u16;

            fn next(&mut self) -> Option<u16> {
                if self.count >= self.len || self.last_result.is_err() {
                    None
                } else {
                    self.last_result = (self.parser)();
                    self.count += 1;
                    self.last_result.as_ref().ok().cloned()
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(self.len))
            }
        }
        impl<Parser: FnMut() -> Result<u16>> std::iter::FusedIterator for CharIter<Parser> {}

        let mut char_iter = CharIter {
            len,
            parser: u16_parser,
            count: 0,
            last_result: Ok(0),
        };
        let mut string = String::with_capacity(len);
        let decode_result = std::char::decode_utf16(&mut char_iter).try_for_each(|maybe_char| {
            match maybe_char {
                Ok(c) => string.push(c),
                Err(e) => {
                    return Err(Error::from(e));
                }
            }
            Ok(())
        });

        char_iter.last_result?;
        decode_result?;

        if Options::STRING_WITH_TERMINATOR {
            if !string.ends_with('\0') {
                return Err(Error::CannotCodeString(
                    "String must end with 0 terminator".into(),
                ));
            }
            string.pop();
        }
        Ok(string)
    }

    fn read_next_tag(&mut self) -> Result<bool> {
        if let SomeIpType::Struct(s) = self.next_type {
            let (wire_type, id) = WireType::disect_tag(u16::deserialize(&mut *self)?);
            let field = s.field_by_id(id);
            if let Some(field) = field {
                field.field_type.get_wire_type().check(wire_type)?;
                self.next_length_field_size = wire_type.get_length_field_size();
                self.next_type = field.field_type;
                self.next_field_name = field.name;
                Ok(true)
            } else {
                if let Some(len) = wire_type.get_fixed_size() {
                    self.discard(len)?;
                } else {
                    self.next_length_field_size = wire_type.get_length_field_size();
                    let lfsize = Options::overwrite_length_field_size(s.length_field_size)
                        .unwrap_or_else(|| {
                            panic!(
                            "Require a length field size to deserialize unknon id {} in struct {}",
                            id, s.name
                        )
                        });
                    let len = self.begin_length_delimited_section(lfsize)?;
                    self.discard(len)?;
                    self.end_length_delimited_section()?;
                }
                Ok(false)
            }
        } else {
            panic!("Can only read tags in TLV encoded structs")
        }
    }
}

impl<'de, Options, Reader> EnumAccess<'de> for &mut SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((seed.deserialize(&mut *self)?, self))
    }
}

impl<'de, Options, Reader> VariantAccess<'de> for &mut SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;

    #[inline]
    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        unimplemented!(
            "Newtype variants (e.g.: enum Foo {{ Bar(u32) }} ) are not supported by someip"
        )
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!(
            "Tuple variants (e.g.: enum Foo {{ Bar(u32, i64, ...) }} ) are not supported by someip"
        )
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!("Struct variants (e.g.: enum Foo {{ Bar{{ a: u32, b: i64, ...}} }} ) are not supported by someip")
    }
}

struct SomeIpSeqAccess<'de: 'a, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    deserializer: &'a mut SomeIpDeserializer<'de, Options, Reader>,
    sequence_type: &'static SomeIpSequence,
    element_count: usize,
}

impl<'de: 'a, 'a, Options, Reader> SomeIpSeqAccess<'de, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    fn new(
        deserializer: &'a mut SomeIpDeserializer<'de, Options, Reader>,
        sequence_type: &'static SomeIpSequence,
    ) -> SomeIpSeqAccess<'de, 'a, Options, Reader> {
        SomeIpSeqAccess {
            deserializer,
            sequence_type,
            element_count: 0,
        }
    }
}

impl<'de: 'a, 'a, Options, Reader> SeqAccess<'de> for SomeIpSeqAccess<'de, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        if self.deserializer.remaining() > 0 {
            self.deserializer.next_type = self.sequence_type.element_type;
            self.deserializer.is_in_tlv_struct = false;
            let element = seed.deserialize(&mut *self.deserializer)?;
            self.element_count += 1;

            if self.element_count > self.sequence_type.max_elements {
                match Options::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA {
                    ActionOnTooMuchData::Fail => {
                        return Err(Error::TooMuchData {
                            max: self.sequence_type.max_elements,
                            actual: 0,
                        })
                    }
                    ActionOnTooMuchData::Discard => {
                        self.deserializer.discard(self.deserializer.remaining())?;
                        return Ok(None);
                    }
                    ActionOnTooMuchData::Keep => {}
                }
            }

            Ok(Some(element))
        } else if self.element_count < self.sequence_type.min_elements {
            Err(Error::NotEnoughData {
                min: self.sequence_type.min_elements,
                actual: self.element_count,
            })
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        if self.sequence_type.element_type.is_const_size() {
            self.sequence_type
                .element_type
                .max_len::<Options>(false)
                .ok()
                .map(|element_len| self.deserializer.remaining() / element_len)
        } else {
            None
        }
    }
}

struct SomeIpStructAccess<'de: 'a, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    deserializer: &'a mut SomeIpDeserializer<'de, Options, Reader>,
    struct_type: &'static SomeIpStruct,
    someip_type: &'static SomeIpType,
    fields: &'static [&'static str],
    in_section: bool,
    was_in_tlv: bool,
    field_index: usize,
}

impl<'de: 'a, 'a, Options, Reader> SomeIpStructAccess<'de, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    #[inline]
    fn begin(
        deserializer: &'a mut SomeIpDeserializer<'de, Options, Reader>,
        struct_type: &'static SomeIpStruct,
        someip_type: &'static SomeIpType,
        fields: &'static [&'static str],
    ) -> Result<Self> {
        let in_section = if let Some(size) =
            struct_type.wanted_length_field::<Options>(deserializer.is_in_tlv_struct)?
        {
            deserializer.begin_length_delimited_section(size)?;
            true
        } else {
            false
        };
        let was_in_tlv = deserializer.is_in_tlv_struct;
        Ok(SomeIpStructAccess {
            deserializer,
            struct_type,
            someip_type,
            fields,
            in_section,
            was_in_tlv,
            field_index: 0,
        })
    }

    #[inline]
    fn end(self) -> Result<()> {
        if self.in_section {
            self.deserializer.end_length_delimited_section()?;
        }
        self.deserializer.is_in_tlv_struct = self.was_in_tlv;
        Ok(())
    }
}

impl<'de: 'a, 'a, Options, Reader> SeqAccess<'de>
    for &mut SomeIpStructAccess<'de, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;

    fn next_element_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<Option<S::Value>> {
        if self.in_section && self.deserializer.remaining() == 0 {
            Ok(None)
        } else {
            if self.field_index >= self.fields.len() {
                panic!(
                    "Cannot deserialize more fields than are known to someip in struct {}",
                    self.struct_type.name
                )
            }
            let field = self
                .struct_type
                .field_by_name(self.fields[self.field_index])
                .unwrap_or_else(|| {
                    panic!(
                        "Struct {} has no field {}",
                        self.struct_type.name, self.fields[self.field_index]
                    )
                });
            self.field_index += 1;
            self.deserializer.next_type = field.field_type;
            self.deserializer.is_in_tlv_struct = false;
            Ok(Some(seed.deserialize(&mut *self.deserializer)?))
        }
    }
}

impl<'de: 'a, 'a, Options, Reader> MapAccess<'de>
    for &mut SomeIpStructAccess<'de, 'a, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;

    fn next_key_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<Option<S::Value>> {
        loop {
            if self.deserializer.remaining() == 0 {
                return Ok(None);
            } else {
                self.deserializer.next_type = self.someip_type;
                if self.deserializer.read_next_tag()? {
                    return Ok(Some(seed.deserialize(&mut *self.deserializer)?));
                }
            }
        }
    }

    fn next_value_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<S::Value> {
        self.deserializer.is_in_tlv_struct = true;
        seed.deserialize(&mut *self.deserializer)
    }
}
impl<'de, Options, Reader> Deserializer<'de> for &mut SomeIpDeserializer<'de, Options, Reader>
where
    Options: SomeIpOptions + ?Sized,
    Reader: SomeIpReader<'de>,
{
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("Someip cannot deserialize unknown data")
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value = self.read_u8()?;
        if Options::DESERIALIZER_STRICT_BOOL && value > 1 {
            return Err(Error::InvalidBool(value));
        }
        visitor.visit_bool(value != 0)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value = self.read_u8()?;
        visitor.visit_i8(value as i8)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value: u16 = self.read_ux(Options::BYTE_ORDER)?;
        visitor.visit_i16(value as i16)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value: u32 = self.read_ux(Options::BYTE_ORDER)?;
        visitor.visit_i32(value as i32)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value: u64 = self.read_ux(Options::BYTE_ORDER)?;
        visitor.visit_i64(value as i64)
    }

    fn deserialize_i128<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("i128 is not supported by someip")
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u8(self.read_u8()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u16(self.read_ux(Options::BYTE_ORDER)?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u32(self.read_ux(Options::BYTE_ORDER)?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u64(self.read_ux(Options::BYTE_ORDER)?)
    }

    fn deserialize_u128<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("u128 is not supported by someip")
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value: u32 = self.read_ux(Options::BYTE_ORDER)?;
        visitor.visit_f32(f32::from_bits(value))
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let value: u64 = self.read_ux(Options::BYTE_ORDER)?;
        visitor.visit_f64(f64::from_bits(value))
    }

    fn deserialize_char<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("char is not supported by someip")
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if Options::STRING_ENCODING == StringEncoding::Utf16 || !Reader::CAN_BORROW {
            //cannot borrow for non utf8 encoding
            return self.deserialize_string(visitor);
        }

        if let SomeIpType::String(s) = self.next_type {
            let len = if let Some(size) = s.wanted_length_field::<Options>(self.is_in_tlv_struct)? {
                self.begin_length_delimited_section(size)?
            } else {
                self.begin_known_length_delimited_section(s.max_size)?
            };

            let mut value = if len < s.min_size {
                return Err(Error::NotEnoughData {
                    min: s.min_size,
                    actual: len,
                });
            } else if len > s.max_size {
                match Options::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA {
                    ActionOnTooMuchData::Fail => {
                        return Err(Error::TooMuchData {
                            max: s.max_size,
                            actual: len,
                        });
                    }
                    ActionOnTooMuchData::Discard => {
                        let val = std::str::from_utf8(self.read_borrowed(s.max_size)?)?;
                        self.discard(len - s.max_size)?;
                        val
                    }
                    ActionOnTooMuchData::Keep => std::str::from_utf8(self.read_borrowed(len)?)?,
                }
            } else {
                std::str::from_utf8(self.read_borrowed(len)?)?
            };

            self.end_length_delimited_section()?;

            if Options::STRING_WITH_BOM {
                if let Some(v) = value.strip_prefix('\u{FEFF}') {
                    value = v;
                } else {
                    return Err(Error::CannotCodeString(
                        "String must begin with a BOM".into(),
                    ));
                }
            }
            if Options::STRING_WITH_TERMINATOR {
                if let Some(v) = value.strip_suffix('\0') {
                    value = v;
                } else {
                    return Err(Error::CannotCodeString(
                        "String must end with 0 terminator".into(),
                    ));
                }
            }
            if Options::STRING_ENCODING == StringEncoding::Ascii && !value.is_ascii() {
                return Err(Error::CannotCodeString(
                    "String contained non ascii chars".into(),
                ));
            }
            visitor.visit_borrowed_str(value)
        } else {
            panic!("Expeceted a string but found {}", self.next_type)
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let SomeIpType::String(s) = self.next_type {
            let len = if let Some(size) = s.wanted_length_field::<Options>(self.is_in_tlv_struct)? {
                self.begin_length_delimited_section(size)?
            } else {
                self.begin_known_length_delimited_section(s.max_size)?
            };

            let value = if len < s.min_size {
                return Err(Error::NotEnoughData {
                    min: s.min_size,
                    actual: len,
                });
            } else if len > s.max_size {
                match Options::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA {
                    ActionOnTooMuchData::Fail => {
                        return Err(Error::TooMuchData {
                            max: s.max_size,
                            actual: len,
                        });
                    }
                    ActionOnTooMuchData::Discard => {
                        let val = if Options::STRING_ENCODING == StringEncoding::Utf16 {
                            self.read_utf16_string(s.max_size)?
                        } else {
                            self.read_utf8_string(s.max_size)?
                        };
                        self.discard(len - s.max_size)?;
                        val
                    }
                    ActionOnTooMuchData::Keep => {
                        if Options::STRING_ENCODING == StringEncoding::Utf16 {
                            self.read_utf16_string(len)?
                        } else {
                            self.read_utf8_string(len)?
                        }
                    }
                }
            } else if Options::STRING_ENCODING == StringEncoding::Utf16 {
                self.read_utf16_string(len)?
            } else {
                self.read_utf8_string(len)?
            };

            self.end_length_delimited_section()?;

            visitor.visit_string(value)
        } else {
            panic!("Expeceted a string but found {}", self.next_type)
        }
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !Reader::CAN_BORROW {
            return self.deserialize_byte_buf(visitor);
        }

        if let SomeIpType::Sequence(s) = self.next_type {
            if !matches!(s.element_type, SomeIpType::Primitive(SomeIpPrimitive::U8)) {
                panic!("Expeceted Primitive(u8) bu found {}", s.element_type)
            }
            let len = if let Some(size) = s.wanted_length_field::<Options>(self.is_in_tlv_struct)? {
                self.begin_length_delimited_section(size)?
            } else {
                self.begin_known_length_delimited_section(s.max_elements)?
            };
            let bytes = if len > s.max_elements {
                match Options::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA {
                    ActionOnTooMuchData::Fail => {
                        return Err(Error::TooMuchData {
                            max: s.max_elements,
                            actual: len,
                        });
                    }
                    ActionOnTooMuchData::Discard => {
                        let b = self.read_borrowed(s.max_elements)?;
                        self.discard(len - s.max_elements)?;
                        b
                    }
                    ActionOnTooMuchData::Keep => self.read_borrowed(len)?,
                }
            } else {
                self.read_borrowed(len)?
            };

            self.end_length_delimited_section()?;

            visitor.visit_borrowed_bytes(bytes)
        } else {
            panic!("Expeceted a sequence of u8s but found {}", self.next_type)
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let SomeIpType::Sequence(s) = self.next_type {
            if !matches!(s.element_type, SomeIpType::Primitive(SomeIpPrimitive::U8)) {
                panic!("Expeceted Primitive(u8) bu found {}", s.element_type)
            }
            let len = if let Some(size) = s.wanted_length_field::<Options>(self.is_in_tlv_struct)? {
                self.begin_length_delimited_section(size)?
            } else {
                self.begin_known_length_delimited_section(s.max_elements)?
            };
            let bytes = if len > s.max_elements {
                match Options::DESERIALIZER_ACTION_ON_TOO_MUCH_DATA {
                    ActionOnTooMuchData::Fail => {
                        return Err(Error::TooMuchData {
                            max: s.max_elements,
                            actual: len,
                        });
                    }
                    ActionOnTooMuchData::Discard => {
                        let b = self.read_vec(s.max_elements)?;
                        self.discard(len - s.max_elements)?;
                        b
                    }
                    ActionOnTooMuchData::Keep => self.read_vec(len)?,
                }
            } else {
                self.read_vec(len)?
            };

            self.end_length_delimited_section()?;

            visitor.visit_byte_buf(bytes)
        } else {
            panic!("Expeceted a sequence of u8s but found {}", self.next_type)
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.is_in_tlv_struct {
            visitor.visit_some(self)
        } else {
            panic!("Options are only supported in tlv structs");
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let SomeIpType::Sequence(s) = self.next_type {
            if let Some(size) = s.wanted_length_field::<Options>(self.is_in_tlv_struct)? {
                self.begin_length_delimited_section(size)?;
            } else {
                self.begin_known_length_delimited_section(
                    s.max_elements * s.element_type.max_len::<Options>(false)?,
                )?;
            }
            let result = visitor.visit_seq(SomeIpSeqAccess::new(self, s))?;
            self.end_length_delimited_section()?;
            Ok(result)
        } else {
            panic!("Expeceted a sequence but found {}", self.next_type)
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value> {
        unimplemented!("Tuples (e.g.: (u32, i64,...) ) are not supported by someip")
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value> {
        unimplemented!(
            "Tuple structs (e.g.: struct Foor(u32, i64, ...) ) are not supported by someip"
        )
    }

    fn deserialize_map<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("Maps are not supported by someip")
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if let SomeIpType::Struct(s) = self.next_type {
            let is_tlv = s.uses_tlv_serialization;
            let mut access = SomeIpStructAccess::begin(self, s, self.next_type, fields)?;
            let result = if is_tlv {
                visitor.visit_map(&mut access)?
            } else {
                visitor.visit_seq(&mut access)?
            };
            access.end()?;
            Ok(result)
        } else {
            panic!("Expeceted a struct but found {}", self.next_type)
        }
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_enum(self)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if !self.next_field_name.is_empty() {
            visitor.visit_str(std::mem::take(&mut self.next_field_name))
        } else if let SomeIpType::Enum(e) = self.next_type {
            let enum_value = match e.get_raw_type() {
                SomeIpPrimitive::U8 => SomeIpEnumValue::U8(u8::deserialize(&mut *self)?),
                SomeIpPrimitive::U16 => SomeIpEnumValue::U16(u16::deserialize(&mut *self)?),
                SomeIpPrimitive::U32 => SomeIpEnumValue::U32(u32::deserialize(&mut *self)?),
                SomeIpPrimitive::U64 => SomeIpEnumValue::U64(u64::deserialize(&mut *self)?),
                SomeIpPrimitive::I8 => SomeIpEnumValue::I8(i8::deserialize(&mut *self)?),
                SomeIpPrimitive::I16 => SomeIpEnumValue::I16(i16::deserialize(&mut *self)?),
                SomeIpPrimitive::I32 => SomeIpEnumValue::I32(i32::deserialize(&mut *self)?),
                SomeIpPrimitive::I64 => SomeIpEnumValue::I64(i64::deserialize(&mut *self)?),
                _ => panic!("Unsupported raw type for enums: {}", e.get_raw_type()),
            };
            let variant = e
                .value_to_name(&enum_value)
                .ok_or_else(|| Error::InvalidEnumValue {
                    value: enum_value.display_value(),
                    name: e.name,
                })?;
            visitor.visit_str(variant)
        } else {
            panic!("Expeceted an enum but found {}", self.next_type)
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        unimplemented!("Someip cannot deserialize unknown data")
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

#[inline]
fn from_internal<'de, Options, T, Reader>(
    reader: Reader,
    someip_type: &'static SomeIpType,
) -> Result<T>
where
    Options: SomeIpOptions + ?Sized,
    T: Deserialize<'de> + ?Sized,
    Reader: SomeIpReader<'de>,
{
    #[cfg(debug_assertions)]
    {
        Options::verify_string_encoding();
        someip_type.verify();
    }

    let mut deserializer = SomeIpDeserializer::<Options, _>::new(reader, someip_type);
    T::deserialize(&mut deserializer)
}

/// Deserialises the value from a [Read].
///
/// # Panics
/// This function panics if the implementation of the [SomeIp](super::SomeIp) trait
/// produces invalid type information or this information is incompatible with the [Deserialize](serde::Deserialize) implementation.
pub fn from_reader<Options, T, Reader>(reader: Reader, len: usize) -> Result<T>
where
    Options: SomeIpOptions + ?Sized,
    T: DeserializeOwned + SomeIp + ?Sized,
    Reader: Read,
{
    from_internal::<Options, T, _>(ExtendedReader::new(reader, len), &T::SOMEIP_TYPE)
}

/// Deserialises the value from a `u8` slice.
///
/// # Panics
/// This function panics if the implementation of the [SomeIp](super::SomeIp) trait
/// produces invalid type information or this information is incompatible with the [Deserialize](serde::Deserialize) implementation.
pub fn from_slice<'a, Options, T>(data: &'a [u8]) -> Result<T>
where
    Options: SomeIpOptions + ?Sized,
    T: Deserialize<'a> + SomeIp + ?Sized,
{
    from_internal::<Options, T, _>(data, &T::SOMEIP_TYPE)
}

#[cfg(feature = "bytes")]
/// Deserialises the value from `Bytes`.
///
/// *Only available with the `bytes` feature.*
///
/// Currently this is just a convenience for [from_slice].
///
/// # Panics
/// This function panics if the implementation of the [SomeIp](super::SomeIp) trait
/// produces invalid type information or this information is incompatible with the [Deserialize](serde::Deserialize) implementation.
#[inline]
pub fn from_bytes<Options, T>(data: bytes::Bytes) -> Result<T>
where
    Options: SomeIpOptions + ?Sized,
    T: DeserializeOwned + SomeIp + ?Sized,
{
    from_slice::<Options, T>(&data)
}

#[test]
fn test_bool() {
    assert_eq!(false, from_slice::<ExampleOptions, bool>(&[0]).unwrap());
    assert_eq!(true, from_slice::<ExampleOptions, bool>(&[1]).unwrap());
}

#[test]
fn test_invalid_bool() {
    assert_eq!(true, from_slice::<ExampleOptions, bool>(&[42]).unwrap());

    struct Options;
    impl SomeIpOptions for Options {
        const DESERIALIZER_STRICT_BOOL: bool = true;
    }

    let res = from_slice::<Options, bool>(&[42]);
    if let Err(Error::InvalidBool(v)) = res {
        assert_eq!(42, v);
    } else {
        assert!(false, "Strict bool parsing must return an error here");
    }
}

#[test]
fn test_u8() {
    assert_eq!(42, from_slice::<ExampleOptions, u8>(&[42]).unwrap());
    assert_eq!(0, from_slice::<ExampleOptions, u8>(&[0]).unwrap());
    assert_eq!(255, from_slice::<ExampleOptions, u8>(&[255]).unwrap());
}

#[test]
fn test_i8() {
    assert_eq!(42, from_slice::<ExampleOptions, i8>(&[42]).unwrap());
    assert_eq!(0, from_slice::<ExampleOptions, i8>(&[0]).unwrap());
    assert_eq!(-1, from_slice::<ExampleOptions, i8>(&[255]).unwrap());
    assert_eq!(127, from_slice::<ExampleOptions, i8>(&[127]).unwrap());
    assert_eq!(-128, from_slice::<ExampleOptions, i8>(&[0x80]).unwrap());
}

#[cfg(test)]
macro_rules! test_endianess {
    ($test_name:ident, $type:ty, $($expected:expr => $to_test:expr),+) => {
        #[test]
        fn $test_name() {
            $(
                {
                    let mut serialized = $to_test;
                    assert_eq!($expected, from_slice::<ExampleOptions, $type>(&serialized).unwrap());
                    serialized.reverse();
                    assert_eq!($expected, from_slice::<test::LittleEndianOptions, $type>(&serialized).unwrap());
                }
            )*
        }
    };
}

#[cfg(test)]
test_endianess!(test_u16, u16,
    42u16 => vec![0, 42],
    0u16 => vec![0, 0],
    u16::max_value() => vec![0xFF, 0xFF]);

#[cfg(test)]
test_endianess!(test_i16, i16,
    42i16 => vec![0, 42],
    0i16 => vec![0, 0],
    i16::max_value() => vec![0x7F, 0xFF],
    -1i16 => vec![0xFF, 0xFF],
    i16::min_value() => vec![0x80, 0]);

#[cfg(test)]
test_endianess!(test_u32, u32,
    42u32 => vec![0, 0, 0, 42],
    0u32 => vec![0; 4],
    u32::max_value() => vec![0xFF; 4]);

#[cfg(test)]
test_endianess!(test_i32, i32,
    42i32 => vec![0, 0, 0, 42],
    0i32 => vec![0; 4],
    i32::max_value() => vec![0x7F, 0xFF, 0xFF, 0xFF],
    -1i32 => vec![0xFF; 4],
    i32::min_value() => vec![0x80, 0, 0, 0]);

#[cfg(test)]
test_endianess!(test_u64, u64,
    42u64 => vec![0, 0, 0, 0, 0, 0, 0, 42],
    0u64 => vec![0; 8],
    u64::max_value() => vec![0xFF; 8]);

#[cfg(test)]
test_endianess!(test_i64, i64,
    42i64 => vec![0, 0, 0, 0, 0, 0, 0, 42],
    0i64 => vec![0; 8],
    i64::max_value() => vec![0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
    -1i64 => vec![0xFF; 8],
    i64::min_value() => vec![0x80, 0, 0, 0, 0, 0, 0, 0]);

#[cfg(test)]
test_endianess!(test_f32, f32,
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
test_endianess!(test_f64, f64,
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
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    assert_eq!(
        TestEnum::First,
        from_slice::<ExampleOptions, TestEnum>(vec![0, 0].as_slice()).unwrap()
    );

    assert_eq!(
        TestEnum::Second,
        from_slice::<ExampleOptions, TestEnum>(vec![0, 42].as_slice()).unwrap()
    );

    assert_eq!(
        TestEnum::Third,
        from_slice::<ExampleOptions, TestEnum>(vec![0xFA, 0xC7].as_slice()).unwrap()
    );
}

#[test]
fn test_bytes() {
    use bytes::{BufMut, Bytes, BytesMut};
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 32,
        min_elements: 0,
        element_type: &u8::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });
    let mut bytes = BytesMut::with_capacity(32);

    bytes.put_u32(0x12345678);
    assert_eq!(
        &bytes,
        &from_internal::<ExampleOptions, Bytes, _>(
            vec![0, 4, 0x12, 0x34, 0x56, 0x78].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );

    bytes.put_u32(0x90ABCDEF);
    assert_eq!(
        &bytes,
        &from_internal::<ExampleOptions, Bytes, _>(
            vec![0, 8, 0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_bytes_no_length_field() {
    use bytes::{BufMut, Bytes, BytesMut};
    const SOMEIP_TYPE: SomeIpType = SomeIpType::Sequence(SomeIpSequence {
        max_elements: 4,
        min_elements: 4,
        element_type: &u8::SOMEIP_TYPE,
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });
    let mut bytes = BytesMut::with_capacity(4);
    bytes.put_u32(0x12345678);
    assert_eq!(
        &bytes,
        &from_internal::<ExampleOptions, Bytes, _>(
            vec![0x12, 0x34, 0x56, 0x78].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_bytes_borrowed() {
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
        &bytes,
        from_internal::<ExampleOptions, &[u8], _>(
            vec![0, 4, 0x12, 0x34, 0x56, 0x78].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );

    bytes.put_u32(0x90ABCDEF);
    assert_eq!(
        &bytes,
        from_internal::<ExampleOptions, &[u8], _>(
            vec![0, 8, 0x12, 0x34, 0x56, 0x78, 0x90, 0xAB, 0xCD, 0xEF].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_bytes_no_length_field_borrowed() {
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
        &bytes,
        from_internal::<ExampleOptions, &[u8], _>(
            vec![0x12, 0x34, 0x56, 0x78].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
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
        values,
        from_internal::<ExampleOptions, Vec<i16>, _>(
            vec![0, 4, 0, 42, 0xFF, 0xFB].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
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
        values,
        from_internal::<ExampleOptions, Vec<i16>, _>(
            vec![0, 42, 0xFF, 0xFB, 0, 1, 0x5, 0x39].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
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
            element_type: &i16::SOMEIP_TYPE,
            length_field_size: Some(LengthFieldSize::OneByte),
        }),
        length_field_size: Some(LengthFieldSize::TwoBytes),
    });

    let values = vec![vec![1i8, -2, 3], vec![42], vec![]];

    assert_eq!(
        values,
        from_internal::<ExampleOptions, Vec<Vec<i8>>, _>(
            vec![0, 7, 3, 1, 0xFE, 3, 1, 42, 0].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_str() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 2,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<ExampleOptions, &str, _>([2, 0x68, 0x69].as_slice(), &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_str_ascii() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_ENCODING: StringEncoding = StringEncoding::Ascii;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 2,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, &str, _>([2, 0x68, 0x69].as_slice(), &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_str_bom() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_BOM: bool = true;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 5,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, &str, _>(
            [5, 0xEF, 0xBB, 0xBF, 0x68, 0x69].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_str_terminator() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_TERMINATOR: bool = true;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 3,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, &str, _>([3, 0x68, 0x69, 0].as_slice(), &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string() {
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 2,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<ExampleOptions, String, _>([2, 0x68, 0x69].as_slice(), &SOMEIP_TYPE)
            .unwrap()
    );
}

#[test]
fn test_string_ascii() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_ENCODING: StringEncoding = StringEncoding::Ascii;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 2,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, String, _>([2, 0x68, 0x69].as_slice(), &SOMEIP_TYPE).unwrap()
    );
}

#[test]
fn test_string_bom() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_BOM: bool = true;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 5,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, String, _>(
            [5, 0xEF, 0xBB, 0xBF, 0x68, 0x69].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_string_terminator() {
    struct Options;
    impl SomeIpOptions for Options {
        const STRING_WITH_TERMINATOR: bool = true;
    }
    const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
        min_size: 0,
        max_size: 3,
        length_field_size: Some(LengthFieldSize::OneByte),
    });
    assert_eq!(
        "hi",
        from_internal::<Options, String, _>([3, 0x68, 0x69, 0].as_slice(), &SOMEIP_TYPE).unwrap()
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
        "hi",
        from_internal::<Options, String, _>(vec![4, 0, 0x68, 0, 0x69].as_slice(), &SOMEIP_TYPE)
            .unwrap()
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
        "hi",
        from_internal::<Options, String, _>(
            vec![6, 0xFE, 0xFF, 0, 0x68, 0, 0x69].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
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
        "hi",
        from_internal::<Options, String, _>(
            vec![6, 0xFF, 0xFE, 0x68, 0, 0x69, 0].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
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
        "hi",
        from_internal::<Options, String, _>(
            vec![6, 0, 0x68, 0, 0x69, 0, 0].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_string_utf16_bom_detect_swapped_byte_order() {
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
        "hi",
        from_internal::<Options, String, _>(
            vec![6, 0xFE, 0xFF, 0, 0x68, 0, 0x69].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_string_utf16_bom_le_detect_swapped_byte_order() {
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
        "hi",
        from_internal::<Options, String, _>(
            vec![6, 0xFF, 0xFE, 0x68, 0, 0x69, 0].as_slice(),
            &SOMEIP_TYPE
        )
        .unwrap()
    );
}

#[test]
fn test_struct() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };

    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Test>(&vec![0xFF, 0xFF, 0, 0, 0, 42, 3, 1, 2, 3]).unwrap()
    );
}

#[test]
fn test_struct_beginning_length_field() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2],
    };
    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Test>(&vec![0, 9, 0xFF, 0xFF, 0, 0, 0, 42, 2, 1, 2, 3])
            .unwrap()
    );
}

#[test]
fn test_struct_tlv() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Test>(&vec![
            0, 16, 0x10, 0x01, 0xFF, 0xFF, 0x20, 0x02, 0, 0, 0, 42, 0x40, 0x03, 3, 1, 2, 3
        ])
        .unwrap(),
    );
}

#[test]
fn test_struct_tlv_message_wrapper() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Test>(&vec![
            0x10, 0x01, 0xFF, 0xFF, 0x20, 0x02, 0, 0, 0, 42, 0x40, 0x03, 3, 1, 2, 3
        ])
        .unwrap(),
    );
}

#[test]
fn test_struct_tlv_alternate_length_field_size() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Test>(&vec![
            0, 17, 0x10, 0x01, 0xFF, 0xFF, 0x20, 0x02, 0, 0, 0, 42, 0x60, 0x03, 0, 3, 1, 2, 3
        ])
        .unwrap(),
    );
}

#[test]
fn test_optional() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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
        Test { a: Some(42) },
        from_slice::<ExampleOptions, Test>(&vec![6, 0x20, 0x00, 0, 0, 0, 42]).unwrap()
    );

    assert_eq!(
        Test { a: None },
        from_slice::<ExampleOptions, Test>(&vec![0]).unwrap()
    );
}

#[test]
fn test_newtype() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    struct Test(String);

    impl SomeIp for Test {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::String(SomeIpString {
            min_size: 0,
            max_size: 42,
            length_field_size: Some(LengthFieldSize::OneByte),
        });
    }

    assert_eq!(
        Test("hi".into()),
        from_slice::<ExampleOptions, Test>(&vec![2, 0x68, 0x69]).unwrap()
    );
}

#[test]
fn test_reader() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let expected = Test {
        first: -1,
        second: 42,
        third: vec![1, 2, 3],
    };
    assert_eq!(
        expected,
        from_reader::<ExampleOptions, Test, _>(
            vec![0, 17, 0x20, 0x02, 0, 0, 0, 42, 0x60, 0x03, 0, 3, 1, 2, 3, 0x10, 0x01, 0xFF, 0xFF]
                .as_slice(),
            19
        )
        .unwrap(),
    );
}

#[test]
fn test_unkown_ids() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
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

    let serialized = vec![
        20, 0x20, 0x01, 1, 2, 3, 4, 0x40, 0x02, 3, 1, 2, 3, 0x60, 0x03, 0, 4, 1, 2, 3, 4,
    ];

    assert_eq!(
        Test { a: None },
        from_slice::<ExampleOptions, Test>(&serialized).unwrap()
    )
}

#[test]
fn test_struct_in_struct() {
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    struct Inner {
        some_field: u32,
    }
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    struct Outer {
        inner: Option<Inner>,
    }

    impl SomeIp for Outer {
        const SOMEIP_TYPE: SomeIpType = SomeIpType::Struct(SomeIpStruct {
            is_message_wrapper: false,
            length_field_size: None,
            name: "Outer",
            uses_tlv_serialization: true,
            fields: &[SomeIpField {
                id: Some(1),
                name: "inner",
                field_type: &SomeIpType::Struct(SomeIpStruct {
                    is_message_wrapper: false,
                    length_field_size: None,
                    name: "Inner",
                    uses_tlv_serialization: true,
                    fields: &[SomeIpField {
                        id: Some(1),
                        name: "some_field",
                        field_type: &u32::SOMEIP_TYPE,
                    }],
                }),
            }],
        });
    }

    let expected = Outer { inner: None };

    let serialized = vec![0, 0, 0, 0];
    assert_eq!(
        expected,
        from_slice::<ExampleOptions, Outer>(&serialized).unwrap()
    );
}
