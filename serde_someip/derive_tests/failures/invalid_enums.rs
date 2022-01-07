use serde_someip::SomeIp;

#[derive(SomeIp)]
#[someip(raw_type = u8)]
enum Test {
    #[someip(value = -1)]
    A,
    B,
    #[someip(value = 1)]
    C {
        foo: u32,
    },
    #[someip(value = 2)]
    D(u32),
    #[someip(valeu = 3)]
    E,
    #[someip(value = 4, foo = bar)]
    F,
    #[someip(value = 256)]
    G,

    #[someip(value = 5)]
    Foo,
    #[someip(value = 5)]
    Bar,
}

#[derive(SomeIp)]
#[someip(raw_type = f32)]
enum Test2 {
    #[someip(value = 0)]
    A,
}

#[derive(SomeIp)]
enum Test3 {
    #[someip(value = 0)]
    A,
}

#[derive(SomeIp)]
#[someip(raw_type = i16, raw_type = i32)]
enum Test4 {
    #[someip(value = 0)]
    A,
}

#[derive(SomeIp)]
#[someip(rwa_type = i16)]
enum Test5 {
    #[someip(value = 0)]
    A,
}

#[derive(SomeIp)]
#[someip(raw_type = i16, foo = bar)]
enum Test6 {
    #[someip(value = 0)]
    A,
}

fn main() {}
