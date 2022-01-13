# `serde_someip` implements [`SOME/IP`](https://www.autosar.org/fileadmin/user_upload/standards/foundation/19-11/AUTOSAR_PRS_SOMEIPProtocol.pdf) ontop of `serde`

```rust
use serde::{Serialize, Deserialize};
use serde_someip::SomeIp;
use serde_someip::options::ExampleOptions;

#[derive(Serialize, Deserialize, Debug, SomeIp)]
struct Point {
    #[someip(id = 0)]
    x: i32,
    #[someip(id = 1)]
    y: i32,
}

fn main() {
    let point = Point { x: 1, y: 2 };

    // Encode the message using someip.
    let serialized = serde_someip::to_vec::<ExampleOptions, _>(&point).unwrap();

    // Prints serialized = [0x20, 0, 0, 0 , 0, 1, 0x20, 1, 0, 0, 0, 2]
    println!("serialized = {:?}", serialized);

    //Decode the message using someip
    let deserialized: Point = serde_someip::from_slice::<ExampleOptions, _>(&serialized).unwrap();

    // Prints deserialized = Point { x: 1, y: 2 }
    println!("deserialized = {:?}", deserialized);
}
```

For more attributes used by `derive(SomeIp)` see the [macro doc](https://docs.rs/serde_someip/latest/serde_someip/derive.SomeIp.html).

## Available features

- `derive`: Enables the `SomeIp` derive macro
- `bytes`: Enables `to/from_bytes` functions using the [bytes](https://crates.io/crates/bytes) crate
