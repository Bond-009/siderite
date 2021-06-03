pub mod auth;
pub mod blocks;
pub mod coord;
pub mod entities;
pub mod server;
pub mod storage;

mod client;
mod protocol;

use std::time::Duration;

/// Number of ticks per second
pub const TPS: i32 = 20;

/// Duration between ticks
pub const TICK_DURATION: Duration = Duration::from_millis(1000 / TPS as u64);
