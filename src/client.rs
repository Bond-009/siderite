use std::sync::{Arc, Mutex, RwLock};
use std::net::TcpStream;

use protocol::Protocol;
use server::Server;

pub struct Client {
    _id: i32,
    username: Option<String>,
    server: Arc<Server>,
    protocol: Option<Arc<Mutex<Protocol>>>
}

impl Client {

    pub fn new(id: i32, server: Arc<Server>, stream: TcpStream) -> Arc<RwLock<Client>> {
        let client = Client {
            _id: id,
            username: None,
            server: server,
            protocol: None
        };
        let ts_client = Arc::new(RwLock::new(client));
        ts_client.write().unwrap().set_protocol(Protocol::new(ts_client.clone(), stream));
        ts_client
    }

    pub fn get_server(&self) -> Arc<Server> {
        self.server.clone()
    }

    pub fn get_username(&self) -> Option<String> {
        self.username.clone()
    }

    fn set_protocol(&mut self, protocol: Protocol) {
        self.protocol = Some(Arc::new(Mutex::new(protocol)));
    }

    pub fn get_protocol(&self) -> Option<Arc<Mutex<Protocol>>> {
        self.protocol.clone()
    }

    pub fn handle_login(&mut self, username: String) {
        self.username = Some(username);
        // TODO: authenticate
    }
}
