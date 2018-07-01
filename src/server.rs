use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, Mutex, RwLock};
use std::{thread, time};

use openssl::pkey::Private;
use openssl::rsa::Rsa;

use client::Client;
use protocol::Protocol;
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

    pub port: u16, // TODO: shouldn't need to be pub

    pub authentication: bool,
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

            authentication: config.authentication,
            public_key_der: rsa.public_key_to_der().unwrap(),
            private_key: rsa
        }
    }

    pub fn start(svr: Arc<Server>) {
        // Tick thread
        let ticksvr = svr.clone();
        thread::spawn(move || {
            loop {
                ticksvr.tick();
                thread::sleep(time::Duration::from_millis(20));
            }
        });

        let listener = TcpListener::bind(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), svr.port)
            ).unwrap();
        info!("Started server");

        for connection in listener.incoming() {
            let consvr = svr.clone();
            thread::spawn(move || {
                info!("Incomming connection!");
                let mut stream = connection.unwrap();
                if Protocol::legacy_ping(&mut stream) {
                    return;
                }

                let protsvr = consvr.clone();
                let mut clients = consvr.clients.lock().unwrap();
                let id = clients.len() as i32;
                let client = Client::new(id, protsvr, stream);
                clients.push(client);
            });
        }
    }

    pub fn tick(&self) {
        let mut clients = self.clients.lock().unwrap();
        for mut client in clients.iter_mut() {
            let prot_p = client.read().unwrap().get_protocol().unwrap();
            let mut prot = prot_p.lock().unwrap();
            prot.process_data();
            prot.handle_packets();
        }
    }

    pub fn online_players(&self) -> i32 {
        let mut players = 0usize;
        for world in &self.worlds {
            players += world.players.len();
        }
        players as i32
    }
}
