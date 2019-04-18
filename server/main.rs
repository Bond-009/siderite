extern crate env_logger;
extern crate siderite;

use std::sync::Arc;

use siderite::server::*;

fn main() {
    env_logger::init();

    let mut server = Server::new(ServerConfig {
        port: 25565,
        description: "A custom MC server".to_owned(),
        max_players: 100,
        favicon: String::new(),
        authentication: false,
    });
    server.load_worlds();
    let server_ref = Arc::new(server);
    Server::start(server_ref.clone());
}
