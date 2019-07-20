pub mod blocks;
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

pub fn var_int_size(value: i32) -> i32 {
    let mut value = value as u32;
    let mut size = 1;
    while value >> 7 != 0 {
        value >>= 7;
        size += 1;
    }

    size
}
