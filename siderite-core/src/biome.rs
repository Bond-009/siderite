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
    FrozenOcean = 10,
    FrozenRiver = 11,
    IcePlains = 12,
    IceMountains = 13,
    MushroomIsland = 14,
    MushroomIslandShore = 15,
    Beach = 16,
    DesertHills = 17,
    ForestHills = 18,
    TaigaHills = 19,
    ExtremeHillsEdge = 20,
    Jungle = 21,
    JungleHills = 22
    // TODO: add more
}
