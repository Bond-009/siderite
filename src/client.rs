use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::Sender;

use uuid::Uuid;
use serde_json as json;

use crate::entities::player::Player;
use crate::protocol::packets::Packet;
use crate::server::Server;
use crate::storage::chunk::*;
use crate::storage::chunk::section::Section;

pub struct Client {
    pub id: i32,
    pub username: Option<String>,
    uuid: Option<Uuid>,
    properties: json::Value,

    _player: Option<Arc<RwLock<Player>>>,

    server: Arc<Server>,
    protocol: Mutex<Sender<Packet>>,
}

impl Client {

    pub fn new(id: i32, server: Arc<Server>, protocol: Sender<Packet>) -> Client {
         Client {
            id: id,
            username: None,
            uuid: None,
            properties: json::Value::Null,

            _player: None,

            server: server,
            protocol: Mutex::new(protocol),
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
        prot.send(Packet::SpawnPosition(world.clone())).unwrap();
        prot.send(Packet::ServerDifficulty()).unwrap();
        prot.send(Packet::PlayerAbilities(player.clone())).unwrap();

        let chunk = ChunkColumn {
            sections: [
                Some(Section {
                    block_types: [3; SECTION_BLOCK_COUNT],
                    block_metas: [0; SECTION_BLOCK_COUNT / 2],
                    block_light: [15; SECTION_BLOCK_COUNT / 2],
                    block_sky_light: [15; SECTION_BLOCK_COUNT / 2]
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None
            ]
        };
        let primary_bit_mask = chunk.get_primary_bit_mask();
        let mut data = Vec::with_capacity(chunk.serialized_size() + 4);
        chunk.serialize(&mut data);

        for x in -3..3 {
            for z in -3..3 {
                prot.send(Packet::ChunkData(x, z, primary_bit_mask, data.clone())).unwrap();
            }
        }

        prot.send(Packet::TimeUpdate(world.clone())).unwrap();
        prot.send(Packet::PlayerPositionAndLook(player.clone())).unwrap();
    }
}
