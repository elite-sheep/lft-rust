// Copyright @yucwang 2022

extern crate pretty_env_logger;
#[macro_use] extern crate log;

fn main() {
    pretty_env_logger::init();

    trace!("Hello, World");
}
