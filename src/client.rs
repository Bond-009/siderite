use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::Sender;

use uuid::Uuid;
use serde_json as json;

use crate::blocks::BlockFace;
use crate::entities::player::Player;
use crate::protocol::DigStatus;
use crate::protocol::packets::Packet;
use crate::server::Server;
use crate::storage::chunk::{ChunkCoord, Coord};

pub struct Client {
    id: i32,
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

    pub fn get_id(&self) -> i32 {
        self.id
    }

    pub fn get_uuid(&self) -> Option<Uuid> {
        self.uuid.clone()
    }

    pub fn kick(&self, reason: &str) {
        let packet = Packet::Disconnect(reason.to_owned());
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
        let chunk_map = world.read().unwrap().get_chunk_map();

        prot.send(Packet::JoinGame(player.clone(), world.clone())).unwrap();
        prot.send(Packet::SpawnPosition(world.clone())).unwrap();
        prot.send(Packet::ServerDifficulty()).unwrap();
        prot.send(Packet::PlayerAbilities(player.clone())).unwrap();

        for x in -3..3 {
            for z in -3..3 {
                let coord = ChunkCoord {x: x, z: z};
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
