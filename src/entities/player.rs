use std::sync::{Arc, RwLock};

use num_derive::FromPrimitive;

use crate::client::Client;
use crate::storage::world::World;

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3
}

const MAX_HEATH: f32 = 20.0;

pub struct Player {
    _client: Arc<RwLock<Client>>,
    world: Arc<RwLock<World>>,

    _health: f32,
    gamemode: GameMode
}

impl Player {
    pub fn new(client: Arc<RwLock<Client>>, world: Arc<RwLock<World>>) -> Player {
        Player { 
            _client: client,
            world,

            _health: MAX_HEATH,
            gamemode: GameMode::Creative // TODO: change
        }
    }

    pub fn get_gamemode(&self) -> GameMode {
        self.gamemode
    }

    pub fn get_world(&self) -> Arc<RwLock<World>> {
        self.world.clone()
    }
}
