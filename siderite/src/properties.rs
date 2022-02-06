use std::net::IpAddr;
use std::str::FromStr;

use siderite_core::entities::player::GameMode;
use siderite_core::server::ServerConfig;
use siderite_core::storage::world::Difficulty;

#[derive(Debug, PartialEq)]
pub struct ServerProperties {
    pub view_distance: u8,
    pub max_building_height: u16,
    pub server_ip: Option<IpAddr>,
    pub level_seed: Option<String>,
    pub gamemode: GameMode,
    pub server_port: u16,
    pub enable_command_block: bool,
    pub allow_nether: bool,
    pub enable_rcon: bool,
    pub op_permission_level: u8,
    pub enable_query: bool,
    pub generator_settings: Option<String>,
    pub resource_pack: Option<String>,
    pub player_idle_timeout: i32,
    pub level_name: String,
    pub motd: String,
    pub announce_player_achievements: bool,
    pub force_gamemode: bool,
    pub hardcore: bool,
    pub white_list: bool,
    pub pvp: bool,
    pub spawn_npcs: bool,
    pub generate_structures: bool,
    pub spawn_animals: bool,
    pub snooper_enabled: bool,
    pub difficulty: Difficulty,
    pub network_compression_threshold: i32,
    pub level_type: String,
    pub spawn_monsters: bool,
    pub max_tick_time: i64,
    pub max_players: i32,
    pub use_native_transport: bool,
    pub spawn_protection: i32,
    pub online_mode: bool,
    pub allow_flight: bool,
    pub resource_pack_hash: Option<String>,
    pub max_world_size: i64
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
            generator_settings: None,
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

impl FromStr for ServerProperties {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        macro_rules! parse {
            ($value:ident, $dest:expr) => {
                if let Ok(v) = $value.parse() {
                    $dest = v;
                }
            }
        }

        macro_rules! parse_optional {
            ($value:ident, $dest:expr) => {
                if let Ok(v) = $value.parse() {
                    $dest = Some(v);
                }
            }
        }

        macro_rules! parse_optional_str {
            ($value:ident, $dest:expr) => {
                if !$value.is_empty() {
                    $dest = Some($value.to_owned());
                }
            }
        }

        let mut properties = ServerProperties::default();
        for (key, value) in s.lines()
                                .filter(|l| !l.starts_with('#'))
                                .map(|l| l.split_once('=').unwrap_or((l, ""))) {
            match key {
                "view-distance" => parse!(value, properties.view_distance),
                "max-build-height" => parse!(value, properties.max_building_height),
                "server-ip" => parse_optional!(value, properties.server_ip),
                "level-seed" => parse_optional_str!(value, properties.level_seed),
                "server-port" => parse!(value, properties.server_port),
                "enable-command-block" => parse!(value, properties.enable_command_block),
                "allow-nether" => parse!(value, properties.allow_nether),
                "gamemode" => {
                    match value {
                        "0" => properties.gamemode = GameMode::Survival,
                        "1" => properties.gamemode = GameMode::Creative,
                        "2" => properties.gamemode = GameMode::Adventure,
                        "3" => properties.gamemode = GameMode::Spectator,
                        _ => {}
                    }
                }
                "enable-rcon" => parse!(value, properties.enable_rcon),
                "enable-query" => parse!(value, properties.enable_query),
                "op-permission-level" => parse!(value, properties.op_permission_level),
                "generator-settings" => parse_optional_str!(value, properties.generator_settings),
                "resource-pack" => parse_optional_str!(value, properties.resource_pack),
                "player-idle-timeout" => parse!(value, properties.player_idle_timeout),
                "level-name" => properties.level_name = value.to_owned(),
                "motd" => properties.motd = value.to_owned(),
                "announce-player-achievements" => parse!(value, properties.announce_player_achievements),
                "force-gamemode" => parse!(value, properties.force_gamemode),
                "white-list" => parse!(value, properties.white_list),
                "pvp" => parse!(value, properties.pvp),
                "spawn-npcs" => parse!(value, properties.spawn_npcs),
                "generate-structures" => parse!(value, properties.generate_structures),
                "spawn-animals" => parse!(value, properties.spawn_animals),
                "snooper-enabled" => parse!(value, properties.snooper_enabled),
                "difficulty" => {
                    match value {
                        "0" => properties.difficulty = Difficulty::Peaceful,
                        "1" => properties.difficulty = Difficulty::Easy,
                        "2" => properties.difficulty = Difficulty::Normal,
                        "3" => properties.difficulty = Difficulty::Hard,
                        _ => {}
                    }
                }
                "network-compression-threshold" => parse!(value, properties.network_compression_threshold),
                "level-type" => properties.level_type = value.to_owned(),
                "spawn-monsters" => parse!(value, properties.spawn_monsters),
                "max-tick-time" => parse!(value, properties.max_tick_time),
                "max-players" => parse!(value, properties.max_players),
                "use-native-transport" => parse!(value, properties.use_native_transport),
                "online-mode" => parse!(value, properties.online_mode),
                "allow-flight" => parse!(value, properties.allow_flight),
                "resource-pack-hash" => parse_optional_str!(value, properties.resource_pack_hash),
                "max-world-size" => parse!(value, properties.max_world_size),
                _ => {}
            }
        }

        Ok(properties)
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
            default_gamemode: properties.gamemode,
            level_name: properties.level_name,
            motd: properties.motd,
            difficulty: properties.difficulty,
            compression_threshold,
            level_type: properties.level_type,
            max_players: properties.max_players,
            encryption: properties.online_mode
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_server_properties() {
        let parsed: ServerProperties = include_str!("../../server.properties").parse().unwrap();
        assert_eq!(parsed, ServerProperties::default());
    }

    #[test]
    fn parse_empty_server_properties() {
        let parsed: ServerProperties = "".parse().unwrap();
        assert_eq!(parsed, ServerProperties::default());
    }
}
