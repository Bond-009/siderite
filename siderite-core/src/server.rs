use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicU32, Ordering};

use log::*;
use openssl::pkey::Private;
use openssl::rsa::Rsa;
use serde_json as json;
use uuid::Uuid;

use crate::client::Client;
use crate::entities::player::{GameMode, Player};
use crate::protocol::Protocol;
use crate::protocol::authenticator::Authenticator;
use crate::protocol::thread::ProtocolThread;
use crate::storage::world::*;

static ENTITY_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn get_next_entity_id() -> u32 {
    ENTITY_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct ServerConfig {
    pub port: u16,
    pub description: String,
    pub max_players: i32,
    pub favicon: String,

    pub authentication: bool
}

pub struct Server {
    id: String,

    // The first world in the vec is the default world
    worlds: Vec<Arc<RwLock<World>>>,
    // Clients that aren't assigned a world yet
    clients: RwLock<HashMap<u32, Arc<RwLock<Client>>>>,

    description: String,
    max_players: i32,
    favicon: String,

    port: u16,

    authenticate: bool,

    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
}

impl Server {

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn max_players(&self) -> i32 {
        self.max_players
    }

    pub fn favicon(&self) -> &str {
        &self.favicon
    }

    pub fn should_authenticate(&self) -> bool {
        self.authenticate
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

    pub fn new(config: ServerConfig) -> Server {
        let rsa = Rsa::generate(1024).unwrap();
        Server {
            // MC Update (1.7.x): The server ID is now sent as an empty string.
            // Hashes also utilize the public key, so they will still be correct.
            id: String::new(),

            worlds: Vec::new(),
            clients: RwLock::new(HashMap::new()),

            description: config.description,
            max_players: config.max_players,
            favicon: config.favicon,

            port: config.port,

            authenticate: config.authentication,

            public_key_der: rsa.public_key_to_der().unwrap(),
            private_key: rsa
        }
    }

    pub fn start(svr: Arc<Server>) {
        info!("Starting siderite on *:{}", svr.port);

        let ps = ProtocolThread::start();
        let auth = Authenticator::start(svr.clone());

        let listener = TcpListener::bind(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), svr.port)
            ).unwrap();

        for connection in listener.incoming() {
            let mut stream = connection.unwrap();
            if Protocol::legacy_ping(&mut stream) {
                return;
            }

            stream.set_nonblocking(true).expect("set_nonblocking call failed");

            let prot = Protocol::new(svr.clone(), stream, auth.clone());
            let (client_id, client) = prot.get_client();
            ps.send(prot).unwrap();

            let mut clients = svr.clients.write().unwrap();
            clients.insert(client_id, client);
        }
    }

    pub fn load_worlds(&mut self) {
        // TODO: change
        self.worlds.push(Arc::new(RwLock::new(World::new(WorldConfig {
            name: "Default".to_owned(),
            dimension: Dimension::Overworld,
            difficulty: Difficulty::Normal,
            default_gamemode: GameMode::Creative
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
            (w.default_gm(), w.spawn_pos())
        };
        let player = Player::new(client_arc2, world, gm, spawn);
        client.finish_auth(Arc::new(RwLock::new(player)));
    }

    pub fn kick_user(&self, client_id: u32, reason: &str) {
        self.do_with_client(client_id, &|client: &Arc<RwLock<Client>>| {
            client.write().unwrap().kick(reason);
            true
        });
    }
}
