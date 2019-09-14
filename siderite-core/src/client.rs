use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::Sender;

use log::*;
use uuid::Uuid;
use serde_json as json;

use crate::blocks::BlockFace;
use crate::entities::player::Player;
use crate::protocol::DigStatus;
use crate::protocol::packets::Packet;
use crate::server::Server;
use crate::coord::{ChunkCoord, Coord};
use crate::storage::world::Difficulty;

pub struct Client {
    id: u32,
    username: Option<String>,
    uuid: Uuid,
    properties: json::Value,

    player: Option<Arc<RwLock<Player>>>,

    server: Arc<Server>,
    protocol: Mutex<Sender<Packet>>,
}

impl Client {

    pub fn new(id: u32, server: Arc<Server>, protocol: Sender<Packet>) -> Client {
         Client {
            id,
            username: None,
            uuid: Uuid::nil(),
            properties: json::Value::Null,

            player: None,

            server,
            protocol: Mutex::new(protocol),
        }
    }

    pub fn server(&self) -> Arc<Server> {
        self.server.clone()
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn properties(&self) -> &json::Value {
        &self.properties
    }

    pub fn get_username(&self) -> Option<&str> {
        match &self.username {
            Some(v) => Some(&v),
            None => None
        }
    }

    pub fn set_username(&mut self, username: String) {
        self.username = Some(username);
    }

    pub fn kick(&self, reason: &str) {
        let packet = Packet::Disconnect(reason.to_owned());
        self.protocol.lock().unwrap().send(packet).unwrap();
    }

    pub fn auth(&mut self, username: String, uuid: Uuid, properties: json::Value) {
        self.username = Some(username);

        if self.uuid.is_nil() {
            self.uuid = uuid;
        }

        if self.properties == json::Value::Null {
            self.properties = properties;
        }

        self.protocol.lock().unwrap().send(Packet::LoginSuccess()).unwrap();
    }

    pub fn finish_auth(&mut self, player: Arc<RwLock<Player>>) {
        self.player = Some(player.clone());
        let world = player.read().unwrap().world();
        let prot = self.protocol.lock().unwrap();
        let chunk_map = world.read().unwrap().chunk_map();

        prot.send(Packet::JoinGame(player.clone(), world.clone())).unwrap();
        prot.send(Packet::SpawnPosition(world.clone())).unwrap();
        prot.send(Packet::ServerDifficulty(Difficulty::Normal)).unwrap();
        prot.send(Packet::PlayerAbilities(player.clone())).unwrap();

        for x in -3..3 {
            for z in -3..3 {
                let coord = ChunkCoord {x, z};
                let map = chunk_map.clone();
                map.touch_chunk(coord);
                prot.send(Packet::ChunkData(
                        coord,
                        map)
                    ).unwrap();
            }
        }

        prot.send(Packet::TimeUpdate(world.clone())).unwrap();
        prot.send(Packet::PlayerPositionAndLook(player.clone())).unwrap();
        prot.send(
            Packet::PlayerListAddPlayer(
                self.server.get_client(self.id).unwrap(),
                player.clone())).unwrap();
    }

    pub fn handle_left_click(&self, _block_pos: Coord<i32>, _face: BlockFace, status: DigStatus) {
        match status {
            DigStatus::StartedDigging => (),
            DigStatus::CancelledDigging => (),
            DigStatus::FinishedDigging => (),
            DigStatus::DropItemStack => (),
            DigStatus::DropItem => (),
            DigStatus::ShootArrowFinishEating => ()
        };
    }
}
