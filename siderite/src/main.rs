#![forbid(unsafe_code)]

mod properties;

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::result::Result;
use std::sync::Arc;

use base64::prelude::*;
use log::*;
use tokio::task;

use siderite_core::auth::*;
use siderite_core::server::*;

use properties::ServerProperties;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROPERTIES_FILENAME: &str = "server.properties";
const FAVICON_FILENAME: &str = "favicon.png";

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Starting siderite version {}", VERSION);

    let favicon = match fs::read(FAVICON_FILENAME) {
        Ok(v) => Some(BASE64_STANDARD_NO_PAD.encode(&v[..])),
        Err(e) => {
            if e.kind() != ErrorKind::NotFound {
                warn!("Error opening favicon file '{}': {}", FAVICON_FILENAME, e);
            }

            None
        }
    };

    info!("Loading properties");
    let properties: ServerProperties = match fs::read_to_string(PROPERTIES_FILENAME) {
        Ok(f) => f.parse().unwrap(),
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                warn!("server.properties does not exist");
            }
            else {
                error!("Failed to load server.properties\n{}", e);
            }

            info!("Generating new properties file");
            Default::default()
        }
    };

    let online = properties.online_mode;

    let listen_addr = SocketAddr::new(
        properties.server_ip.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED)),
        properties.server_port);
    let (tx, rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(
        properties.into(),
        favicon,
        tx);

    server.load_worlds();

    let server = Arc::new(server);
    let server_ref = server.clone();

    let authenticator = get_authenticator(if online { "mojang" } else { "offline" });
    task::spawn(async move {
        for m in rx.iter() {
            match authenticator.authenticate(m).await {
                Ok(o) => server_ref.auth_user(o.client_id, o.username, o.uuid, o.properties),
                Err(e) => error!("Failed auth with {:?}", e)
            }
        }
    });

    Server::start(server, listen_addr);

    Ok(())
}

fn get_authenticator(authenticator: &str) -> Box<dyn Authenticator> {
    #[cfg(feature = "mojang_auth")]
    if authenticator == "mojang" {
        return Box::new(siderite_mojang::MojangAuthenticator::new()) as Box<dyn Authenticator>;
    }

    if !authenticator.is_empty() && authenticator != "offline" {
        warn!("Unknown authenticator: {}", authenticator);
    }

    warn!("**** SERVER IS RUNNING IN OFFLINE MODE!");
    Box::new(OfflineAuthenticator) as Box<dyn Authenticator>
}
