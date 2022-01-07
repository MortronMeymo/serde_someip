use serde_someip::SomeIp;

#[derive(SomeIp)]
struct TestUnit;

#[derive(SomeIp)]
struct TestUnnamed();

#[derive(SomeIp)]
struct TestNamed {}

fn main() {}
