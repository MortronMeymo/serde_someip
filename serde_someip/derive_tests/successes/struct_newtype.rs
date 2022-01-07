use serde_someip::SomeIp;

#[derive(SomeIp)]
struct TestPrimitive(u32);

#[derive(SomeIp)]
#[someip(max_size = 42)]
struct TestString(String);

#[derive(SomeIp)]
#[someip(min_size = 10, max_size = 10)]
struct TestString2<'a>(&'a str);

#[derive(SomeIp)]
#[someip(min_size = 0, max_size = 42, length_field_size = 1)]
struct TestString3<'a>(&'a String);

#[derive(SomeIp)]
struct TestArray([u8; 10]);

#[derive(SomeIp)]
#[someip(max_elements = 42)]
struct TestSlice<'a>(&'a [f32]);

#[derive(SomeIp)]
#[someip(min_elements = 10, max_elements = 42, length_field_size = 2)]
struct TestVec(Vec<i64>);

#[derive(SomeIp)]
#[someip(max_elements = 42, elements = (max_size = 1337))]
struct TestVecString(Vec<String>);

#[derive(SomeIp)]
#[someip(max_elements = 42, elements = (max_elements = 3, elements = (max_size = 1337)))]
struct TestMultiDimVec(Vec<Vec<String>>);

#[derive(SomeIp)]
#[someip(treat_as = [u8], max_elements = 123)]
struct TestBytes(bytes::Bytes);

fn main() {
    use serde_someip::length_fields::LengthFieldSize;
    use serde_someip::types::*;
    assert_eq!(u32::SOMEIP_TYPE, TestPrimitive::SOMEIP_TYPE);

    assert_eq!(
        SomeIpType::String(SomeIpString {
            min_size: 0,
            max_size: 42,
            length_field_size: None,
        }),
        TestString::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::String(SomeIpString {
            min_size: 10,
            max_size: 10,
            length_field_size: None,
        }),
        TestString2::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::String(SomeIpString {
            min_size: 0,
            max_size: 42,
            length_field_size: Some(LengthFieldSize::OneByte)
        }),
        TestString3::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Sequence(SomeIpSequence {
            min_elements: 10,
            max_elements: 10,
            element_type: &u8::SOMEIP_TYPE,
            length_field_size: None,
        }),
        TestArray::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Sequence(SomeIpSequence {
            min_elements: 0,
            max_elements: 42,
            element_type: &f32::SOMEIP_TYPE,
            length_field_size: None,
        }),
        TestSlice::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Sequence(SomeIpSequence {
            min_elements: 10,
            max_elements: 42,
            element_type: &i64::SOMEIP_TYPE,
            length_field_size: Some(LengthFieldSize::TwoBytes),
        }),
        TestVec::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Sequence(SomeIpSequence {
            min_elements: 0,
            max_elements: 42,
            length_field_size: None,
            element_type: &SomeIpType::String(SomeIpString {
                min_size: 0,
                max_size: 1337,
                length_field_size: None
            },)
        }),
        TestVecString::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Sequence(SomeIpSequence {
            min_elements: 0,
            max_elements: 123,
            length_field_size: None,
            element_type: &u8::SOMEIP_TYPE,
        },),
        TestBytes::SOMEIP_TYPE,
    );
}
