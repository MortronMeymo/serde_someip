use serde_someip::SomeIp;

#[derive(SomeIp)]
#[someip(raw_type = i16)]
enum Test {
    #[someip(value = 0)]
    A,
    #[someip(value = 1)]
    B,
    #[someip(value = -1)]
    C,
}

fn main() {
    use serde_someip::types::*;
    assert_eq!(
        SomeIpType::Enum(SomeIpEnum {
            name: "Test",
            raw_type: SomeIpPrimitive::I16,
            values: &[
                ("A", SomeIpEnumValue::I16(0)),
                ("B", SomeIpEnumValue::I16(1)),
                ("C", SomeIpEnumValue::I16(-1))
            ],
        }),
        Test::SOMEIP_TYPE
    );
}
