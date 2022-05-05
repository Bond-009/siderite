use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use num_derive::FromPrimitive;

use crate::coord::Coord;
use crate::entities::player::Player;
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
    pub spawn_pos: Coord<i32>
}

pub struct World {
    _name: String,
    dimension: Dimension,

    players: HashMap<u32, Arc<RwLock<Player>>>,
    chunk_map: Arc<ChunkMap>,

    spawn_pos: Coord<i32>
}

impl World {
    pub fn new(config: WorldConfig) -> Self {
        Self {
            _name: config.name,
            dimension: config.dimension,
            spawn_pos: config.spawn_pos,

            players: HashMap::new(),
            chunk_map: Arc::new(ChunkMap::new())
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    pub fn chunk_map(&self) -> Arc<ChunkMap> {
        self.chunk_map.clone()
    }

    /// Returns the default spawn position for this world
    pub fn spawn_pos(&self) -> Coord<i32> {
        self.spawn_pos
    }

    pub fn foreach_player(&self, function: &dyn Fn(&Arc<RwLock<Player>>)) {
        for player in self.players.values() {
            function(&player);
        }
    }

    pub fn add_player(&mut self, id: u32, player: Arc<RwLock<Player>>) {
        self.players.insert(id, player);
    }

    pub fn remove_player(&mut self, id: u32) -> Option<Arc<RwLock<Player>>> {
        self.players.remove(&id)
    }
}
