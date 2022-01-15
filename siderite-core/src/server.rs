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
use crate::protocol::thread::ProtocolThread;
use crate::storage::world::*;

static ENTITY_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn get_next_entity_id() -> u32 {
    ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct ServerConfig {
    pub view_distance: u8,
    pub motd: String,
    pub difficulty: Difficulty,
    pub compression_threshold: Option<i32>,
    pub max_players: i32,
    pub encryption: bool
}

pub struct Server {
    id: String,

    // The first world in the vec is the default world
    worlds: Vec<Arc<RwLock<World>>>,
    // Clients that aren't assigned a world yet
    clients: RwLock<HashMap<u32, Arc<RwLock<Client>>>>,

    motd: String,
    difficulty: Difficulty,
    compression_threshold: Option<i32>,
    max_players: i32,
    favicon: Option<String>,

    encryption: bool,

    pub authenticator: Sender<AuthInfo>,

    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
}

impl Server {

    pub fn motd(&self) -> &str {
        &self.motd
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
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

            motd: config.motd,
            difficulty: config.difficulty,
            compression_threshold: config.compression_threshold,
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

    pub fn remove_client(&self, client_id: u32) {
        let mut clients = self.clients.write().unwrap();
        clients.remove(&client_id);
        debug!("Removed client with id: {}", client_id);
    }

    pub fn load_worlds(&mut self) {
        // TODO: change
        self.worlds.push(Arc::new(RwLock::new(World::new(WorldConfig {
            name: "world".to_owned(),
            dimension: Dimension::Overworld,
            default_gamemode: GameMode::Creative,
            random_seed: 0,
            generator_name: "default".into(),
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

    pub fn foreach_client(&self, function: &dyn Fn(&Arc<RwLock<Client>>)) {
        let clients = self.clients.read().unwrap();
        for client in clients.values() {
            function(client);
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
        client.auth(username, uuid, properties);
        let world = self.default_world();
        let (gm, spawn) = {
            let w = world.read().unwrap();
            (w.default_gamemode(), w.spawn_pos())
        };
        let player = Player::new(client_arc2, world, gm, spawn.into());
        client.finish_auth(Arc::new(RwLock::new(player)));
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
        self.foreach_client(&|client: &Arc<RwLock<Client>>| {
            client.read().unwrap().send_chat(raw_msg.clone());
        });
    }
}
