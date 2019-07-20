#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum Biome {
    Ocean = 0,
    Plains = 1,
    Desert = 2,
    ExtremeHills = 3,
    Forest = 4,
    Taiga = 5,
    Swampland = 6,
    River = 7,
    Nether = 8,
    End = 9,
    // TODO: add more
}
