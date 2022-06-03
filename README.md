# LFT-Rust

A lock-free threadpool implementation in Rust. Research oriented. We developed an interesting 
idea to mitigate the performance drop introduced by the use of locks in Rust. For details, please
refer to our [technical reports](https://drive.google.com/file/d/1f8S21PyhDJ5uu-Xr90JMmNEbrJBWHpZh/view?usp=sharing).

## Build

This project is built upon `Rust 1.57.0`. To build this project, please run the following
command:  
```
cargo build
```

To build a release version of this project, please run:  
```
cargo build --release
```

We also provide some examples for better using this project, to run an example:
```
RUST_LOG=trace cargo run --example hello_world
```

## Roadmap

I plan to implement the following features:  

- [ ] Move common objects to a shared mod
- [ ] Better scheduling algorithms: Round-robin, weighted Round-robin.  
- [ ] Multi-lane threadpool
- [ ] Load rebalancing
- [ ] Function binding: https://en.cppreference.com/w/cpp/utility/functional/bind
- [ ] Lock-free channel
