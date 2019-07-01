use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, Mutex, RwLock};
use std::{thread};

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

pub struct ServerConfig {
    pub port: u16,
    pub description: String,
    pub max_players: i32,
    pub favicon: String,

    pub authentication: bool
}

pub struct Server {
    pub id: String,

    // The first world in the vec is the default world
    pub worlds: Vec<Arc<RwLock<World>>>,
    // Clients that aren't assigned a world yet
    clients: Mutex<Vec<Arc<RwLock<Client>>>>,

    pub description: String,
    pub max_players: i32,
    pub favicon: String,

    port: u16,

    pub authenticate: bool,

    pub public_key_der: Vec<u8>,
    pub private_key: Rsa<Private>
}

impl Server {

    pub fn new(config: ServerConfig) -> Server {
        let rsa = Rsa::generate(1024).unwrap();
        Server {
            // MC Update (1.7.x): The server ID is now sent as an empty string.
            // Hashes also utilize the public key, so they will still be correct.
            id: String::new(),

            worlds: Vec::new(),
            clients: Mutex::new(Vec::new()),

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
            let consvr = svr.clone();
            let ps_c = ps.clone();
            let auth_c = auth.clone();
            thread::spawn(move || {
                info!("Incomming connection!");
                let mut stream = connection.unwrap();
                if Protocol::legacy_ping(&mut stream) {
                    return;
                }

                stream.set_nonblocking(true).expect("set_nonblocking call failed");

                let mut clients = consvr.clients.lock().unwrap();
                let client_id = clients.len() as i32;
                let prot = Protocol::new(client_id, consvr.clone(), stream, auth_c);
                let client = prot.get_client();
                ps_c.send(prot).unwrap();

                clients.push(client);
            });
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

    fn get_client(&self, client_id: i32) -> Option<Arc<RwLock<Client>>> {
        for client in self.clients.lock().unwrap().iter() {
            if client.read().unwrap().get_id() == client_id {
                return Some(client.clone());
            }
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

    pub fn auth_user(&self, client_id: i32, username: String, uuid: Uuid, properties: json::Value) {
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

    pub fn kick_user(&self, client_id: i32, reason: &str) {
        for client in self.clients.lock().unwrap().iter() {
            if client.read().unwrap().get_id() == client_id {
                client.write().unwrap().kick(reason);
                return;
            }
        }
    }
}
