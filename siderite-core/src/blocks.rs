use num_derive::FromPrimitive;

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum BlockType {
    Air = 0,
    Stone = 1,
    Grass = 2,
    Dirt = 3,
    CobbleStone = 4,
    // TODO: Add more
}

#[repr(i8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum BlockFace {
    /// -Y
    YM = 0,
    /// +Y
    YP = 1,
    /// -Z
    ZM = 2,
    /// +Z
    ZP = 3,
    /// -X
    XM = 4,
    /// +X
    XP = 5,
}
