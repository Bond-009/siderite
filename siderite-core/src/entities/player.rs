use std::sync::{Arc, RwLock};

use bitflags::bitflags;
use num_derive::FromPrimitive;

use crate::client::Client;
use crate::coord::Coord;
use crate::storage::world::World;

bitflags! {
    #[derive(Default)]
    pub struct Abilities: u8 {

        /// Invulnerable.
        const INVULNERABLE = 0x01;

        /// Flying.
        const FLYING = 0x02;

        /// Allow Flying.
        const MAY_FLY = 0x04;

        /// Creative mode.
        const CREATIVE = 0x08;
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3
}

/// Default amount of health for a player
/// This is the max value when regenerating
/// The health value can be larger than this due to commands
const DEFAULT_HEATH: f32 = 20.0;

pub struct Player {
    client: Arc<RwLock<Client>>,
    world: Arc<RwLock<World>>,

    health: f32,
    gamemode: GameMode,
    is_flying: bool,
    may_fly: bool,
    pos: Coord<f64>
}

impl Player {
    pub fn new(
        client: Arc<RwLock<Client>>,
        world: Arc<RwLock<World>>,
        gamemode: GameMode,
        pos: Coord<f64>) -> Self
    {
        Self {
            client,
            world,

            gamemode,
            health: DEFAULT_HEATH,
            is_flying: false,
            may_fly: gamemode == GameMode::Creative || gamemode == GameMode::Spectator,
            pos
        }
    }

    /// Returns the current gamemode of the player.
    pub fn gamemode(&self) -> GameMode {
        self.gamemode
    }

    pub fn world(&self) -> Arc<RwLock<World>> {
        self.world.clone()
    }

    pub fn client(&self) -> Arc<RwLock<Client>> {
        self.client.clone()
    }

    pub fn health(&self) -> f32 {
        self.health
    }

    pub fn abilities(&self) -> Abilities {
        let mut abilities = Abilities::default();
        if self.gamemode == GameMode::Creative {
            abilities |= Abilities::INVULNERABLE | Abilities::CREATIVE;
        }

        if self.is_flying {
            abilities |= Abilities::FLYING;
        }

        if self.may_fly {
            abilities |= Abilities::MAY_FLY;
        }

        abilities
    }

    pub fn pos(&self) -> Coord<f64> {
        self.pos
    }
}
