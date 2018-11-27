#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum Blocks {
    Air = 0,
    Stone = 1,
    Grass = 2,
    Dirt = 3,
    CobbleStone = 4,
}
