use player::Player;

pub struct WorldConfig {
    name: String
}

pub struct World {
    pub players: Vec<Player>,

    name: String
}

impl World {

    pub fn new(config: WorldConfig) -> World {
        World {
            players: Vec::new(),

            name: config.name
        }
    }
}
