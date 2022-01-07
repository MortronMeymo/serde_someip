use serde_someip::SomeIp;

#[derive(SomeIp)]
union Test {
    a: u32,
    b: i64,
    c: u8,
}

fn main() {}
