use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread;

use openssl::pkey::Private;
use openssl::rsa::Rsa;

use protocol::Protocol;
use world::World;

pub struct ServerConfig {
    pub port: u16,
    pub description: String,
    pub max_players: i32,
    pub favicon: String
}

pub struct Server {
    // The first world in the vec is the default world
    pub worlds: Vec<World>,

    pub description: String,
    pub max_players: i32,
    pub favicon: String,

    pub port: u16, // TODO: shouldn't need to be pub

    pub public_key_der: Vec<u8>,
    pub private_key: Rsa<Private>
}

impl Server {

    pub fn new(config: ServerConfig) -> Server {
        let rsa = Rsa::generate(1024).unwrap();
        Server {
            worlds: Vec::new(),

            description: config.description,
            max_players: config.max_players,
            favicon: config.favicon,

            port: config.port,

            public_key_der: rsa.public_key_to_der().unwrap(),
            private_key: rsa
        }
    }

    pub fn start(svr: Arc<Server>) {

        let listener = TcpListener::bind(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), svr.port)
            ).unwrap();
        info!("Started server");

        for connection in listener.incoming() {
            let svr2 = svr.clone();
            thread::spawn(|| {
                info!("Incomming connection!");
                let stream = connection.unwrap();
                let mut proto = Protocol::new(stream, svr2);
                proto.data_received();
            });
        }
    }

    pub fn online_players(&self) -> i32 {
        let mut players = 0i32;
        for world in &self.worlds {
            players += world.players.len() as i32;
        }
        players
    }
}
