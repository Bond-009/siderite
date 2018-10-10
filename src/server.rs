use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, Mutex, RwLock};
use std::{thread};

use openssl::pkey::Private;
use openssl::rsa::Rsa;
use serde_json as json;
use uuid::Uuid;

use client::Client;
use protocol::Protocol;
use protocol::authenticator::Authenticator;
use protocol::thread::ProtocolThread;
use world::World;

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
    pub worlds: Vec<World>,
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
            id: "-".to_owned(), // TODO: Generate random ID

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

                let mut clients = consvr.clients.lock().unwrap();
                let client_id = clients.len() as i32;
                let prot = Protocol::new(client_id, consvr.clone(), stream, auth_c);
                let client = prot.get_client();
                ps_c.send(prot).unwrap();
                
                clients.push(client);
            });
        }
    }

    pub fn online_players(&self) -> i32 {
        let mut players = 0usize;
        for world in &self.worlds {
            players += world.players.len();
        }
        players as i32
    }

    pub fn auth_user(&self, client_id: i32, username: String, uuid: Uuid, properties: json::Value) {
        if self.online_players() >= self.max_players {
            self.kick_user(client_id, "The server is currently full.");
            return;
        }

        for client in self.clients.lock().unwrap().iter() {
            if client.read().unwrap().id == client_id {
                client.write().unwrap().auth(username, uuid, properties);
                return;
            }
        }
    }

    pub fn kick_user(&self, client_id: i32, reason: &str) {
        for client in self.clients.lock().unwrap().iter() {
            if client.read().unwrap().id == client_id {
                client.write().unwrap().kick(reason);
                return;
            }
        }
    }
}
