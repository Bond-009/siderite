use std::sync::{Arc, RwLock};

use crossbeam_channel::Sender;
use uuid::Uuid;
use serde_json as json;

use crate::auth::AuthInfo;
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
    protocol: Sender<Packet>,
}

impl Client {

    pub fn new(id: u32, server: Arc<Server>, protocol: Sender<Packet>) -> Self {
         Self {
            id,
            username: None,
            uuid: Uuid::nil(),
            properties: json::Value::Null,

            player: None,

            server,
            protocol,
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
        self.username.as_deref()
    }

    pub fn set_username(&mut self, username: String) {
        self.username = Some(username);
    }

    pub fn kick(&self, reason: &str) {
        self.protocol.send(Packet::Disconnect(reason.to_owned())).unwrap();
    }

    pub fn handle_login(&self, server_id: Option<String>) {
        self.server.authenticator.send(AuthInfo {
            client_id: self.id,
            server_id,
            username: self.username.as_ref().expect("expected username").to_owned()
        }).unwrap();
    }

    pub fn auth(&mut self, username: String, uuid: Uuid, properties: json::Value) {
        self.username = Some(username);

        if self.uuid.is_nil() {
            self.uuid = uuid;
        }

        if self.properties == json::Value::Null {
            self.properties = properties;
        }

        self.protocol.send(Packet::LoginSuccess()).unwrap();
    }

    pub fn finish_auth(&mut self, player: Arc<RwLock<Player>>) {
        self.player = Some(player.clone());
        let world = player.read().unwrap().world();
        let chunk_map = world.read().unwrap().chunk_map();

        self.protocol.send(Packet::JoinGame(player.clone(), world.clone())).unwrap();
        self.protocol.send(Packet::SpawnPosition(world.clone())).unwrap();
        self.protocol.send(Packet::ServerDifficulty(Difficulty::Normal)).unwrap();
        self.protocol.send(Packet::PlayerAbilities(player.clone())).unwrap();

        for x in -3..3 {
            for z in -3..3 {
                let coord = ChunkCoord {x, z};
                let map = chunk_map.clone();
                map.touch_chunk(coord);
                self.protocol.send(Packet::ChunkData(
                        coord,
                        map)
                    ).unwrap();
            }
        }

        self.protocol.send(Packet::TimeUpdate(world)).unwrap();
        self.protocol.send(Packet::PlayerPositionAndLook(player.clone())).unwrap();
        self.protocol.send(
            Packet::PlayerListAddPlayer(
                self.server.get_client(self.id).unwrap(),
                player)).unwrap();
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

    pub fn send_chat(&self, raw_msg: String) {
        self.protocol.send(Packet::ChatMessage(raw_msg)).unwrap();
    }
}
