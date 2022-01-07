//! Provides the [LengthFieldSize] enum.
//!
//! This is needed since the someip standard does not fully define
//! how to encode length fields and every type can make different choices here.

use std::cmp::{Ord, Ordering};

///The possible length field sizes supported by SomeIp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthFieldSize {
    /// A length field with a size of one byte.
    OneByte,
    /// A length field with a size of two bytes.
    TwoBytes,
    /// A length field with a size of four bytes.
    FourBytes,
}

impl LengthFieldSize {
    pub(crate) fn minimum_length_for(len: usize) -> LengthFieldSize {
        if len <= u8::max_value() as usize {
            LengthFieldSize::OneByte
        } else if len <= u16::max_value() as usize {
            LengthFieldSize::TwoBytes
        } else if len <= u32::max_value() as usize {
            LengthFieldSize::FourBytes
        } else {
            panic!("Cannot handle message with len={} bytes", len)
        }
    }
}

impl Ord for LengthFieldSize {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            LengthFieldSize::OneByte => match other {
                LengthFieldSize::OneByte => Ordering::Equal,
                LengthFieldSize::TwoBytes => Ordering::Less,
                LengthFieldSize::FourBytes => Ordering::Less,
            },
            LengthFieldSize::TwoBytes => match other {
                LengthFieldSize::OneByte => Ordering::Greater,
                LengthFieldSize::TwoBytes => Ordering::Equal,
                LengthFieldSize::FourBytes => Ordering::Less,
            },
            LengthFieldSize::FourBytes => match other {
                LengthFieldSize::OneByte => Ordering::Greater,
                LengthFieldSize::TwoBytes => Ordering::Greater,
                LengthFieldSize::FourBytes => Ordering::Equal,
            },
        }
    }
}

impl PartialOrd for LengthFieldSize {
    fn partial_cmp(&self, rhs: &LengthFieldSize) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl From<LengthFieldSize> for usize {
    fn from(v: LengthFieldSize) -> usize {
        match v {
            LengthFieldSize::OneByte => 1,
            LengthFieldSize::TwoBytes => 2,
            LengthFieldSize::FourBytes => 4,
        }
    }
}

impl std::fmt::Display for LengthFieldSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LengthFieldSize::OneByte => formatter.write_str("1"),
            LengthFieldSize::TwoBytes => formatter.write_str("2"),
            LengthFieldSize::FourBytes => formatter.write_str("4"),
        }
    }
}

#[test]
fn test_ord_impl() {
    assert!(LengthFieldSize::OneByte <= LengthFieldSize::OneByte);
    assert!(LengthFieldSize::TwoBytes <= LengthFieldSize::TwoBytes);
    assert!(LengthFieldSize::FourBytes <= LengthFieldSize::FourBytes);
    assert!(LengthFieldSize::OneByte < LengthFieldSize::TwoBytes);
    assert!(LengthFieldSize::OneByte < LengthFieldSize::FourBytes);
    assert!(LengthFieldSize::TwoBytes < LengthFieldSize::FourBytes);

    assert!(LengthFieldSize::TwoBytes > LengthFieldSize::OneByte);
    assert!(LengthFieldSize::FourBytes > LengthFieldSize::OneByte);
    assert!(LengthFieldSize::FourBytes > LengthFieldSize::TwoBytes);
}
