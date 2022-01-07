use serde_someip::SomeIp;

//is missing derive(SomeIp) or manual implementation of trait
enum SomeEnum {
    A,
    B,
}

#[derive(SomeIp)]
struct Test(SomeEnum);

#[derive(SomeIp)]
#[someip(max_elements = 42)]
struct Test2(Vec<String>);

#[derive(SomeIp)]
#[someip(max_elements = 42, elements = ())]
struct Test3(Vec<String>);

#[derive(SomeIp)]
#[someip(max_elements = 42, elements = ())]
struct Test4(Vec<Vec<u8>>);

#[derive(SomeIp)]
#[someip(elements = ())]
struct Test5([String; 6]);

fn main() {}
