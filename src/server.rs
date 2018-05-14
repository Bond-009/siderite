use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::thread;

use protocol::Protocol;

pub struct Server {
    /*
    port: u16,
    description: String,
    max_players: i32,
    favicon: String,*/
}

impl Server {
    pub fn new() -> Server {
        Server { }
    }

    pub fn start(&self) {
        let listener = TcpListener::bind(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 25565)
            ).unwrap();
        println!("Started server");

        for connection in listener.incoming() {
            thread::spawn(|| {
                println!("Incomming connection!");
                let stream = connection.unwrap();
                let mut proto = Protocol::new(stream);
                proto.data_received();
            });
        }
    }
}
