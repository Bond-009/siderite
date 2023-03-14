use std::sync::{Arc, RwLock};

use crate::coord::ChunkCoord;
use crate::entities::player::Player;
use crate::protocol::GameStateReason;
use crate::storage::chunk::chunk_map::ChunkMap;
use crate::storage::world::{Difficulty, World};

#[derive(Clone)]
pub enum Packet {
    // Login state
    ///
    LoginSuccess(),

    // Play state
    /// Raw Chat Message
    ChatMessage(String),
    /// Player, World
    JoinGame(Arc<RwLock<Player>>, Arc<RwLock<World>>),
    /// World
    TimeUpdate(Arc<RwLock<World>>),
    /// World
    SpawnPosition(Arc<RwLock<World>>),
    /// Player
    PlayerPositionAndLook(Arc<RwLock<Player>>),
    /// Player
    SpawnPlayer(Arc<RwLock<Player>>),
    /// PlayerListAction, Players
    PlayerListItem(PlayerListAction, Box<[Arc<RwLock<Player>>]>),
    /// Player
    PlayerAbilities(Arc<RwLock<Player>>),
    /// Primary Bit Mask, Chunk Data
    ChunkData(ChunkCoord, Arc<ChunkMap>),
    /// Difficulty
    ServerDifficulty(Difficulty),
    ///
    ChangeGameState(GameStateReason, f32),
    ///
    ResourcePackSend(String, String),

    // Other
    /// Reason
    Disconnect(String),
}

#[repr(i32)]
#[derive(Copy, Clone, Debug)]
pub enum PlayerListAction {
    AddPlayer = 0,
    UpdateGamemode = 1,
    UpdateLatency = 2,
    UpdateDisplayName = 3,
    RemovePlayer = 4
}
