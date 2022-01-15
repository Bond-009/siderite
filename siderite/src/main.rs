#![forbid(unsafe_code)]

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::result::Result;
use std::sync::Arc;

use log::*;
use siderite_core::entities::player::GameMode;
use siderite_core::storage::world::Difficulty;
use tokio::task;

use siderite_core::auth::*;
use siderite_core::server::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PROPERTIES_FILENAME: &str = "server.properties";
const FAVICON_FILENAME: &str = "favicon.png";

#[allow(unused)]
struct ServerProperties {
    view_distance: u8,
    max_building_height: u16,
    server_ip: Option<IpAddr>,
    level_seed: Option<i64>,
    gamemode: GameMode,
    server_port: u16,
    enable_command_block: bool,
    allow_nether: bool,
    enable_rcon: bool,
    op_permission_level: u8,
    enable_query: bool,
    generator_setting: Option<String>,
    resource_pack: Option<String>,
    player_idle_timeout: i32,
    level_name: String,
    motd: String,
    announce_player_achievements: bool,
    force_gamemode: bool,
    hardcore: bool,
    white_list: bool,
    pvp: bool,
    spawn_npcs: bool,
    generate_structures: bool,
    spawn_animals: bool,
    snooper_enabled: bool,
    difficulty: Difficulty,
    network_compression_threshold: i32,
    level_type: String,
    spawn_monsters: bool,
    max_tick_time: i32,
    max_players: i32,
    use_native_transport: bool,
    spawn_protection: i32,
    online_mode: bool,
    allow_flight: bool,
    resource_pack_hash: Option<String>,
    max_world_size: i64
}

impl Default for ServerProperties {
    fn default() -> Self {
        ServerProperties {
            view_distance: 10,
            max_building_height: 256,
            server_ip: None,
            level_seed: None,
            gamemode: GameMode::Survival,
            server_port: 25565,
            enable_command_block: false,
            allow_nether: true,
            enable_rcon: false,
            op_permission_level: 4,
            enable_query: false,
            generator_setting: None,
            resource_pack: None,
            player_idle_timeout: 0,
            level_name: "world".to_owned(),
            motd: "A Minecraft Server".to_owned(),
            announce_player_achievements: true,
            force_gamemode: false,
            hardcore: false,
            white_list: false,
            pvp: true,
            spawn_npcs: true,
            generate_structures: true,
            spawn_animals: true,
            snooper_enabled: true,
            difficulty: Difficulty::Easy,
            network_compression_threshold: 256,
            level_type: "DEFAULT".to_owned(),
            spawn_monsters: true,
            max_tick_time: 60000,
            max_players: 20,
            use_native_transport: true,
            spawn_protection: 16,
            online_mode: true,
            allow_flight: false,
            resource_pack_hash: None,
            max_world_size: 29999984
        }
    }
}

impl From<ServerProperties> for ServerConfig {
    fn from(properties: ServerProperties) -> ServerConfig {
        let compression_threshold = if properties.network_compression_threshold < 0 {
            None
        }
        else {
            Some(properties.network_compression_threshold)
        };

        ServerConfig {
            view_distance: properties.view_distance,
            motd: properties.motd,
            compression_threshold,
            max_players: properties.max_players,
            encryption: properties.online_mode
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Starting siderite version {}", VERSION);

    let favicon = match fs::read(FAVICON_FILENAME) {
        Ok(v) => Some(base64::encode(&v)),
        Err(e) => {
            if e.kind() != ErrorKind::NotFound {
                warn!("Error opening favicon file '{}': {}", FAVICON_FILENAME, e);
            }

            None
        }
    };

    // TODO: read properties file
    let properties = ServerProperties::default();

    let listen_addr = SocketAddr::new(
        properties.server_ip.unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED)),
        properties.server_port);
    let (tx, rx) = crossbeam_channel::unbounded();

    let mut server = Server::new(
        properties.into(),
        favicon,
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

    Server::start(server_ref, listen_addr);

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
