use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU32, Ordering};

use crossbeam_channel::Sender;
use log::*;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use serde_json as json;
use uuid::Uuid;

use crate::auth::*;
use crate::client::Client;
use crate::coord::Coord;
use crate::entities::player::{GameMode, Player};
use crate::protocol::Protocol;
use crate::protocol::packets::{Packet, PlayerListAction};
use crate::protocol::thread::ProtocolThread;
use crate::storage::world::*;

static ENTITY_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn get_next_entity_id() -> u32 {
    ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct ServerConfig {
    pub view_distance: u8,
    pub default_gamemode: GameMode,
    pub level_name: String,
    pub motd: String,
    pub difficulty: Difficulty,
    pub compression_threshold: Option<i32>,
    pub level_type: String,
    pub max_players: i32,
    pub encryption: bool
}

pub struct Server {
    id: String,

    // The first world in the vec is the default world
    worlds: Vec<Arc<RwLock<World>>>,
    // Clients that aren't assigned a world yet
    clients: RwLock<HashMap<u32, Arc<RwLock<Client>>>>,

    default_gamemode: GameMode,
    level_name: String,
    motd: String,
    difficulty: Difficulty,
    compression_threshold: Option<i32>,
    level_type: String,
    max_players: i32,
    favicon: Option<String>,

    encryption: bool,

    pub authenticator: Sender<AuthInfo>,

    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
}

impl Server {

    /// Returns the default gamemode.
    pub fn default_gamemode(&self) -> GameMode {
        self.default_gamemode
    }

    pub fn motd(&self) -> &str {
        &self.motd
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
    }

    pub fn level_type(&self) -> &str {
        &self.level_type
    }

    pub fn max_players(&self) -> i32 {
        self.max_players
    }

    pub fn favicon(&self) -> Option<&str> {
        self.favicon.as_deref()
    }

    pub fn encryption(&self) -> bool {
        self.encryption
    }

    pub fn private_key(&self) -> &Rsa<Private> {
        &self.private_key
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn public_key_der(&self) -> &[u8] {
        &self.public_key_der
    }

    pub fn new(config: ServerConfig, favicon: Option<String>, authenticator: Sender<AuthInfo>) -> Server {
        let rsa = Rsa::generate(1024).unwrap();
        Server {
            // MC Update (1.7.x): The server ID is now sent as an empty string.
            // Hashes also utilize the public key, so they will still be correct.
            id: String::new(),

            worlds: Vec::new(),
            clients: RwLock::new(HashMap::new()),

            default_gamemode: config.default_gamemode,
            level_name: config.level_name,
            motd: config.motd,
            difficulty: config.difficulty,
            compression_threshold: config.compression_threshold,
            level_type: config.level_type,
            max_players: config.max_players,
            encryption: config.encryption,

            favicon,

            authenticator,

            public_key_der: rsa.public_key_to_der().unwrap(),
            private_key: rsa
        }
    }

    pub fn start(svr: Arc<Server>, address: SocketAddr) {
        info!("Starting siderite on {}", address);

        let ps = ProtocolThread::start();

        let listener = TcpListener::bind(address).unwrap();
        for connection in listener.incoming() {
            let mut stream = connection.unwrap();
            if Protocol::legacy_ping(&mut stream) {
                return;
            }

            stream.set_nonblocking(true).expect("set_nonblocking call failed");
            stream.set_nodelay(true).expect("set_nodeley call failed");

            let prot = Protocol::new(svr.clone(), stream);
            let (client_id, client) = prot.get_client();
            ps.send(prot).unwrap();

            let mut clients = svr.clients.write().unwrap();
            clients.insert(client_id, client);
            debug!("Added client with id: {}", client_id);
        }
    }

    pub fn remove_client(&self, id: u32) {
        let mut clients = self.clients.write().unwrap();
        if clients.remove(&id).is_some() {
            return;
        }
        let mut player = None;
        for world in &self.worlds {
            if let Some(v) = world.write().unwrap().remove_player(id) {
                player = Some(v);
                break;
            }
        }

        if let Some(player) = player {
            let client = player.read().unwrap().client();
            let client = client.read().unwrap();
            let msg = format!("{} left the game", client.get_username().unwrap());
            info!("{}", msg);
            self.broadcast(Packet::ChatMessage(msg));
            self.broadcast(Packet::PlayerListItem(PlayerListAction::RemovePlayer, Box::new([player])));
        }
    }

    pub fn load_worlds(&mut self) {
        // TODO: change
        self.worlds.push(Arc::new(RwLock::new(World::new(WorldConfig {
            name: self.level_name.clone(),
            dimension: Dimension::Overworld,
            spawn_pos: Coord::<i32>::new(0, 65, 0)
        }))));
    }

    pub fn default_world(&self) -> Arc<RwLock<World>> {
        self.worlds[0].clone()
    }

    pub fn do_with_client(&self, client_id: u32, function: &dyn Fn(&Arc<RwLock<Client>>) -> bool) -> bool {
        let clients = self.clients.read().unwrap();

        if let Some(client) = clients.get(&client_id) {
            return function(client);
        }

        false
    }

    pub fn foreach_player(&self, function: &dyn Fn(&Arc<RwLock<Player>>)) {
        for world in &self.worlds {
            world.read().unwrap().foreach_player(function);
        }
    }

    pub fn get_client(&self, client_id: u32) -> Option<Arc<RwLock<Client>>> {
        let clients = self.clients.read().unwrap();

        if let Some(client) = clients.get(&client_id) {
            return Some(client.clone());
        }

        None
    }

    pub fn online_players(&self) -> i32 {
        let mut players = 0usize;
        for world in &self.worlds {
            players += world.read().unwrap().num_players();
        }

        players as i32
    }

    pub fn auth_user(&self, client_id: u32, username: String, uuid: Uuid, properties: json::Value) {
        if self.online_players() >= self.max_players {
            self.kick_user(client_id, "The server is currently full.");
            return;
        }

        let client_arc = self.get_client(client_id).unwrap();
        let client_arc2 = client_arc.clone();

        let mut client = client_arc.write().unwrap();
        let join_message = format!("{} joined the game", username);
        client.auth(username, uuid, properties);
        // TODO: get correct world for player
        let world = self.default_world();
        let spawn = {
            let w = world.read().unwrap();
            w.spawn_pos()
        };
        let player = Player::new(client_arc2, world.clone(), self.default_gamemode(), spawn.into());
        let player_arc = Arc::new(RwLock::new(player));

        info!("{}", join_message);
        self.broadcast(Packet::ChatMessage(join_message));
        client.finish_auth(player_arc.clone());

        self.remove_client(client_id);
        world.write().unwrap().add_player(client_id, player_arc);
    }

    pub fn kick_user(&self, client_id: u32, reason: &str) {
        self.do_with_client(client_id, &|client: &Arc<RwLock<Client>>| {
            client.read().unwrap().kick(reason);
            true
        });
    }

    pub fn broadcast_chat(&self, username: &str, msg: &str) {
        let raw_msg = format!("<{}>: {}", username, msg);
        info!("{}", raw_msg);
        self.broadcast(Packet::ChatMessage(raw_msg));
    }

    pub fn broadcast(&self, packet: Packet) {
        self.foreach_player(&|player| {
            player.read().unwrap().client().read().unwrap().send(packet.clone());
        });
    }
}
