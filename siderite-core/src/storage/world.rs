use std::sync::Arc;

use num_derive::FromPrimitive;

use crate::coord::Coord;
use crate::entities::player::{GameMode, Player};
use crate::storage::chunk::chunk_map::ChunkMap;

#[repr(i8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum Dimension {
    Nether = -1,
    Overworld = 0,
    End = 1
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3
}

pub struct WorldConfig {
    pub name: String,
    pub dimension: Dimension,
    pub difficulty: Difficulty,
    pub default_gamemode: GameMode
}

pub struct World {
    _name: String,
    dimension: Dimension,
    difficulty: Difficulty,

    players: Vec<Player>,
    chunk_map: Arc<ChunkMap>,

    default_gm: GameMode,
    spawn_pos: Coord<f64>
}

impl World {
    pub fn new(config: WorldConfig) -> World {
        World {
            _name: config.name,
            dimension: config.dimension,
            difficulty: config.difficulty,

            players: Vec::new(),
            chunk_map: Arc::new(ChunkMap::new()),

            default_gm: config.default_gamemode,
            spawn_pos: Coord::new(0.0, 0.0, 0.0)
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    pub fn chunk_map(&self) -> Arc<ChunkMap> {
        self.chunk_map.clone()
    }

    /// Returns the default gamemode for this world
    pub fn default_gm(&self) -> GameMode {
        self.default_gm
    }

    /// Returns the default spawn position for this world
    pub fn spawn_pos(&self) -> Coord<f64> {
        self.spawn_pos
    }
}
