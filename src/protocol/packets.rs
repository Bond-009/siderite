use std::sync::{Arc, RwLock};

use crate::entities::player::Player;
use crate::protocol::GameStateReason;
use crate::storage::world::World;
use crate::storage::chunk::chunk_map::{ChunkCoord, ChunkMap};

pub enum Packet {
    // Login state
    ///
    LoginSuccess(),

    // Play state
    /// Player, World
    JoinGame(Arc<RwLock<Player>>, Arc<RwLock<World>>),
    /// World
    TimeUpdate(Arc<RwLock<World>>),
    /// World
    SpawnPosition(Arc<RwLock<World>>),
    /// Player
    PlayerPositionAndLook(Arc<RwLock<Player>>),
    /// Player
    PlayerAbilities(Arc<RwLock<Player>>),
    /// Primary Bit Mask, Chunk Data
    ChunkData(ChunkCoord, Arc<ChunkMap>),
    ///
    ServerDifficulty(),
    ///
    ChangeGameState(GameStateReason, f32),

    // Other
    /// Reason
    Disconnect(String),
}
