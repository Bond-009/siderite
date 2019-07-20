use std::io::Read;
use std::fs::File;
use std::sync::Arc;

use base64::encode;
use env_logger;
use log::*;
use siderite_core::server::*;

const FAVICON_FILENAME: &'static str = "favicon.png";

fn main() {
    env_logger::init();

    let favicon: String = match File::open(FAVICON_FILENAME) {
        Ok(mut v) => {
            let mut bytes = Vec::new();
            v.read_to_end(&mut bytes).unwrap();
            encode(&bytes)
        },
        Err(e) => {
            warn!("Error opening favicon file '{}': {}", FAVICON_FILENAME, e);
            String::new()
        }
    };

    let mut server = Server::new(ServerConfig {
        port: 25565,
        description: "A custom MC server".to_owned(),
        max_players: 10,
        favicon,
        authentication: false,
    });
    server.load_worlds();
    let server_ref = Arc::new(server);
    Server::start(server_ref.clone());
}
