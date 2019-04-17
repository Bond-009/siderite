use crate::entities::player::Player;

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
    pub difficulty: Difficulty
}

pub struct World {
    _name: String,
    dimension: Dimension,
    difficulty: Difficulty,

    players: Vec<Player>,
}

impl World {
    pub fn new(config: WorldConfig) -> World {
        World {
            _name: config.name,
            dimension: config.dimension,
            difficulty: config.difficulty,

            players: Vec::new(),
        }
    }

    pub fn get_dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn get_difficulty(&self) -> Difficulty {
        self.difficulty
    }

    pub fn get_num_players(&self) -> usize {
        self.players.len()
    }
}
