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
use crate::entities::player::Player;
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
    private_key: Rsa<Private>
}

impl Server {

    pub fn get_description<'a>(&'a self) -> &'a str {
        &self.description
    }

    pub fn get_max_players(&self) -> i32 {
        self.max_players
    }

    pub fn get_favicon<'a>(&'a self) -> &'a str {
        &self.favicon
    }

    pub fn should_authenticate(&self) -> bool {
        self.authenticate
    }

    pub fn get_private_key<'a>(&'a self) -> &'a Rsa<Private> {
        &self.private_key
    }

    pub fn get_id<'a>(&'a self) -> &'a str {
        &self.id
    }

    pub fn get_public_key_der<'a>(&'a self) -> &'a [u8] {
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
        let ps = ProtocolThread::start();
        let auth = Authenticator::start(svr.clone());

        let listener = TcpListener::bind(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), svr.port)
            ).unwrap();
        info!("Started server");

        for connection in listener.incoming() {
            info!("Incomming connection!");
            let mut stream = connection.unwrap();
            if Protocol::legacy_ping(&mut stream) {
                return;
            }

            stream.set_nonblocking(true).expect("set_nonblocking call failed");

            let prot = Protocol::new(svr.clone(), stream, auth.clone());
            let client = prot.get_client();
            ps.send(prot).unwrap();

            let mut clients = svr.clients.write().unwrap();
            clients.insert(client.0, client.1);
        }
    }

    pub fn load_worlds(&mut self) {
        // TODO: change
        self.worlds.push(Arc::new(RwLock::new(World::new(WorldConfig {
            name: "Default".to_owned(),
            dimension: Dimension::Overworld,
            difficulty: Difficulty::Normal
        }))));
    }

    pub fn try_load_player(&self, client: Arc<RwLock<Client>>) -> Arc<RwLock<Player>>{
        // TODO: Try load player

        let world = self.worlds[0].clone();
        Arc::new(RwLock::new(Player::new(client, world)))
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
            players += world.read().unwrap().get_num_players();
        }

        players as i32
    }

    pub fn auth_user(&self, client_id: u32, username: String, uuid: Uuid, properties: json::Value) {
        if self.online_players() >= self.max_players {
            self.kick_user(client_id, "The server is currently full.");
            return;
        }

        info!("Authenticated user {}", username);

        let client_arc = self.get_client(client_id).unwrap();
        let client_arc_clone = client_arc.clone();
        let mut client = client_arc.write().unwrap();
        client.auth(username, uuid, properties);
        let player = self.try_load_player(client_arc_clone);
        client.finish_auth(player);
    }

    pub fn kick_user(&self, client_id: u32, reason: &str) {
        self.do_with_client(client_id, &|client: &Arc<RwLock<Client>>| {
            client.write().unwrap().kick(reason);
            true
        });
    }
}
