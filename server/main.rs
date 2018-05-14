extern crate siderite;
/*
#[macro_use]
extern crate log;*/

use siderite::server::*;

fn main() {
    let server = Server::new();
    server.start();
}
