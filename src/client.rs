use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::Sender;

use uuid::Uuid;
use serde_json as json;

use entities::player::Player;
use protocol::authenticator::AuthInfo;
use protocol::packets::Packet;
use server::Server;

pub struct Client {
    pub id: i32,
    pub username: Option<String>,
    uuid: Option<Uuid>,
    properties: json::Value,

    _player: Option<Arc<RwLock<Player>>>,

    server: Arc<Server>,
    protocol: Mutex<Sender<Packet>>,
    authenticator: Mutex<Sender<AuthInfo>>
}

impl Client {

    pub fn new(id: i32, server: Arc<Server>, protocol: Sender<Packet>, authenticator: Sender<AuthInfo>) -> Client {
         Client {
            id: id,
            username: None,
            uuid: None,
            properties: json::Value::Null,

            _player: None,

            server: server,
            protocol: Mutex::new(protocol),
            authenticator: Mutex::new(authenticator)
        }
    }

    pub fn get_server(&self) -> Arc<Server> {
        self.server.clone()
    }

    pub fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.clone()
    }

    pub fn kick(&self, reason: &str) {
        let packet = Packet::Disconnect(reason.to_string());
        self.protocol.lock().unwrap().send(packet).unwrap();
    }

    pub fn handle_login(&mut self, username: String) {
        self.username = Some(username.clone());

        self.authenticator.lock().unwrap().send(AuthInfo {
            client_id: self.id,
            username: username
        }).unwrap();
    }

    pub fn auth(&mut self, username: String, uuid: Uuid, properties: json::Value) {
        self.username = Some(username);

        if self.uuid == None {
            self.uuid = Some(uuid);
        }

        if self.properties == json::Value::Null {
            self.properties = properties;
        }

        self.protocol.lock().unwrap().send(Packet::LoginSuccess()).unwrap();
    }

    pub fn finish_auth(&self, player: Arc<RwLock<Player>>) {
        let world = player.read().unwrap().get_world();
        let prot = self.protocol.lock().unwrap();
        prot.send(Packet::JoinGame(player.clone(), world.clone())).unwrap();
        prot.send(Packet::SpawnPosition(world)).unwrap();
        prot.send(Packet::ServerDifficulty()).unwrap();
        prot.send(Packet::PlayerAbilities(player)).unwrap();
    }
}
