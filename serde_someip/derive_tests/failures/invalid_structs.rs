use serde_someip::SomeIp;

#[derive(SomeIp)]
struct NeitherTlvNorNonTlv {
    a: u32,
    #[someip(id = 1)]
    b: u32,
}

#[derive(SomeIp)]
struct InvalidIds {
    #[someip(id = 1)]
    a: u32,
    #[someip(id = 1)]
    b: u32,
    #[someip(id = 0x1000)]
    c: u32,
    #[someip(id = -1)]
    d: u32,
}

#[derive(SomeIp)]
struct OptionInNonTlvStruct {
    a: i16,
    b: Option<f64>,
}

fn main() {}
