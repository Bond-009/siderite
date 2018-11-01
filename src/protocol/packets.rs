use std::sync::{Arc, RwLock};

use entities::player::Player;
use world::World;

pub enum Packet {
    // Login state
    /// UUID, Username
    LoginSuccess(),

    // Play state
    /// Player, World
    JoinGame(Arc<RwLock<Player>>, Arc<RwLock<World>>),
    /// World
    SpawnPosition(Arc<RwLock<World>>),
    /// Player
    PlayerAbilities(Arc<RwLock<Player>>),
    ///
    ServerDifficulty(),

    // Other
    /// Reason
    Disconnect(String),
}
