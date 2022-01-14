use std::convert::From;

use num_traits::Num;

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct Coord<T: Num + PartialOrd + Copy> {
    pub x: T,
    pub y: T,
    pub z: T
}

impl<T: Num + PartialOrd + Copy> Coord<T> {
    pub fn new(x: T, y: T, z: T) -> Self {
        Coord { x, y, z }
    }
}

// TODO: Make generic
impl From<Coord<i32>> for Coord<f64> {
    fn from(other: Coord<i32>) -> Coord<f64> {
        Coord {
            x: other.x.into(),
            y: other.y.into(),
            z: other.z.into()
        }
    }
}
