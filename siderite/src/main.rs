use std::io::Read;
use std::fs::File;
use std::error::Error;
use std::result::Result;
use std::sync::Arc;

use log::*;
use tokio::task;

use siderite_core::auth::*;
use siderite_core::server::*;

const FAVICON_FILENAME: &str = "favicon.png";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Starting siderite version {}", VERSION);
    let favicon: String = match File::open(FAVICON_FILENAME) {
        Ok(mut v) => {
            let mut bytes = Vec::new();
            v.read_to_end(&mut bytes)?;
            base64::encode(&bytes)
        },
        Err(e) => {
            warn!("Error opening favicon file '{}': {}", FAVICON_FILENAME, e);
            String::new()
        }
    };

    let (tx, rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(
        ServerConfig {
            port: 25565,
            description: "A Minecraft server".to_owned(),
            max_players: 20,
            favicon,
            encryption: true,
        },
        tx);

    server.load_worlds();

    let server_ref = Arc::new(server);
    let server_ref2 = server_ref.clone();

    let authenticator = get_authenticator("mojang");
    task::spawn(async move {
        for m in rx.iter() {
            match authenticator.authenticate(m).await {
                Ok(o) => server_ref2.auth_user(o.client_id, o.username, o.uuid, o.properties),
                Err(e) => error!("Failed auth with {:?}", e)
            }
        }
    });

    Server::start(server_ref);

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
    return Box::new(OfflineAuthenticator) as Box<dyn Authenticator>;
}
