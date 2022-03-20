use serde_someip::SomeIp;

#[derive(SomeIp)]
struct NonTlvTest {
    a: i16,
    b: f64,
    #[someip(max_elements = 42)]
    c: Vec<u8>,
    #[someip(max_size = 1337)]
    d: String,
}

#[derive(SomeIp)]
struct TlvTest {
    #[someip(id = 1)]
    a: i16,
    #[someip(id = 2)]
    b: f64,
    #[someip(id = 3, max_elements = 42)]
    c: Vec<u8>,
    #[someip(id = 4, max_size = 1337)]
    d: String,
}

#[derive(SomeIp)]
#[someip(message_wrapper = true, length_field_size = 2)]
struct MessageWrapperTest {
    foo: u32,
}

#[derive(SomeIp)]
struct OptionTest {
    #[someip(id = 1)]
    a: i16,
    #[someip(id = 2)]
    b: Option<f64>,
}

#[derive(SomeIp)]
#[someip(strings_length_field_size = 2, structs_length_field_size = 1)]
struct PropsTest {
    a: i16,
}

fn main() {
    use serde_someip::length_fields::LengthFieldSize;
    use serde_someip::types::*;

    assert_eq!(
        SomeIpType::Struct(SomeIpStruct {
            name: "NonTlvTest",
            uses_tlv_serialization: false,
            is_message_wrapper: false,
            length_field_size: None,
            transformation_properties: None,
            fields: &[
                SomeIpField {
                    name: "a",
                    id: None,
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "b",
                    id: None,
                    field_type: &f64::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "c",
                    id: None,
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        min_elements: 0,
                        max_elements: 42,
                        length_field_size: None,
                        element_type: &u8::SOMEIP_TYPE,
                    }),
                },
                SomeIpField {
                    name: "d",
                    id: None,
                    field_type: &SomeIpType::String(SomeIpString {
                        min_size: 0,
                        max_size: 1337,
                        length_field_size: None,
                    }),
                }
            ],
        }),
        NonTlvTest::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Struct(SomeIpStruct {
            name: "TlvTest",
            uses_tlv_serialization: true,
            is_message_wrapper: false,
            length_field_size: None,
            transformation_properties: None,
            fields: &[
                SomeIpField {
                    name: "a",
                    id: Some(1),
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "b",
                    id: Some(2),
                    field_type: &f64::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "c",
                    id: Some(3),
                    field_type: &SomeIpType::Sequence(SomeIpSequence {
                        min_elements: 0,
                        max_elements: 42,
                        length_field_size: None,
                        element_type: &u8::SOMEIP_TYPE,
                    }),
                },
                SomeIpField {
                    name: "d",
                    id: Some(4),
                    field_type: &SomeIpType::String(SomeIpString {
                        min_size: 0,
                        max_size: 1337,
                        length_field_size: None,
                    }),
                }
            ],
        }),
        TlvTest::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Struct(SomeIpStruct {
            name: "MessageWrapperTest",
            uses_tlv_serialization: false,
            is_message_wrapper: true,
            length_field_size: Some(LengthFieldSize::TwoBytes),
            transformation_properties: None,
            fields: &[SomeIpField {
                name: "foo",
                id: None,
                field_type: &u32::SOMEIP_TYPE,
            }],
        }),
        MessageWrapperTest::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Struct(SomeIpStruct {
            name: "OptionTest",
            uses_tlv_serialization: true,
            is_message_wrapper: false,
            length_field_size: None,
            transformation_properties: None,
            fields: &[
                SomeIpField {
                    name: "a",
                    id: Some(1),
                    field_type: &i16::SOMEIP_TYPE,
                },
                SomeIpField {
                    name: "b",
                    id: Some(2),
                    field_type: &f64::SOMEIP_TYPE,
                },
            ],
        }),
        OptionTest::SOMEIP_TYPE
    );

    assert_eq!(
        SomeIpType::Struct(SomeIpStruct {
            name: "PropsTest",
            uses_tlv_serialization: false,
            is_message_wrapper: false,
            length_field_size: None,
            transformation_properties: Some(SomeIpTransforationProperties {
                size_of_array_length_field: None,
                size_of_struct_length_field: Some(LengthFieldSize::OneByte),
                size_of_string_length_field: Some(LengthFieldSize::TwoBytes),
            }),
            fields: &[SomeIpField {
                name: "a",
                id: None,
                field_type: &i16::SOMEIP_TYPE,
            }]
        }),
        PropsTest::SOMEIP_TYPE
    );
}
