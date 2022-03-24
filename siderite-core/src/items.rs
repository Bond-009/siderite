use num_derive::FromPrimitive;

#[repr(u16)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
pub enum BlockType {
    Air = 0,
    Stone = 1,
    Grass = 2,
    Dirt = 3,
    CobbleStone = 4,
    Planks = 5,
    Sapling = 6,
    Bedrock = 7,
    FlowingWater = 8,
    Water = 9,
    FlowingLava = 10,
    Lava = 11,
    Sand = 12,
    Gravel = 13,
    GoldOre = 14,
    IronOre = 15,
    CoalOre = 16,
    Log = 17,
    Leaves = 18,
    Sponge = 19,
    Glass = 20,
    LapisOre = 21,
    LapisBlock = 22,
    Dispenser = 23,
    SandStone = 24,
    NoteBlock = 25,
    Bed = 26,
    PoweredRail = 27,
    DetectorRail = 28,
    StickyPiston = 29,
    Cobweb = 30,
    TallGrass = 31,
    DeadBush = 32,
    Piston = 33,
    PistonHead = 34,
    Wool = 35,
    PistonExtension = 36,
    Dandelion = 37,
    Flower = 38,
    BrownMushroom = 39,
    RedMushroom = 40,
    GoldBlock = 41,
    IronBlock = 42,
    DoubleStoneSlab = 43,
    StoneSlab = 44,
    Brick = 45,
    Tnt = 46,
    Bookshelf = 47,
    MossyCobblestone = 48,
    Obsidian = 49,
    Torch = 50,
    Fire = 51,
    MobSpawner = 52,
    OakStairs = 53,
    Chest = 54,
    RedstoneWire = 55,
    DiamondOre = 56,
    DiamondBlock = 57,
    CraftingTable = 58,
    Wheat = 59,
    Farmland = 60,
    Furnace = 61,
    LitFurnace = 62,
    StandingSign = 63,
    WoodenDoor = 64,
    Ladder = 65,
    Rail = 66,
    CobbleStoneStairs = 67,
    WallSign = 68,
    Lever = 69,
    StonePressurePlate = 70,
    IronDoor = 71,
    WoodenPressurePlate = 72,
    RedstoneOre = 73,
    LitRedstoneOre = 74,
    UnlitRedstoneTorch = 75,
    RedstoneTorch = 76,
    StoneButton = 77,
    SnowLayer = 78,
    Ice = 79,
    Snow = 80,
    Cactus = 81,
    Clay = 82,
    Reeds = 83,
    Jukebox = 84,
    Fence = 85,
    Pumpkin = 86,
    Netherrack = 87,
    SoulSand = 88,
    Glowstone = 89,
    Portal = 90,
    LitPumpkin = 91,
    Cake = 92,
    UnpoweredRepeater = 93,
    PoweredRepeater = 94,
    StainedGlass = 95,
    Trapdoor = 96,
    MonsterEgg = 97,
    Stonebrick = 98,
    BrownMushroomBlock = 99,
    RedMushroomBlock = 100,
    IronBars = 101,
    GlassPane = 102,
    MelonBlock = 103,
    PumpkinStem = 104,
    MelonStem = 105,
    Vine = 106,
    FenceGate = 107,
    BrickStairs = 108,
    StoneBrickStairs = 109,
    Mycelium = 110,
    Waterlily = 111,
    NetherBrick = 112,
    NetherBrickFence = 113,
    NetherBrickStairs = 114,
    NetherWart= 115,
    EnchantingTable = 116,
    BrewingStand = 117,
    Cauldron = 118,
    EndPortal = 119,
    EndPortalFrame = 120,
    EndStone = 121,
    DragonEgg = 122,
    RedstoneLamp = 123,
    LitRedstoneLamp = 124,
    DoubleWoodenSlab = 125,
    WoodenSlab = 126,
    Cocoa = 127,
    SandstoneStairs =128,
    EmeraldOre = 129,
    EnderChest = 130,
    TripwireHook = 131,
    Tripwire = 132,
    EmeraldBlock = 133,
    SpruceStairs = 134,
    BriceStairs = 135,
    JungleStairs = 136,
    CommandBlock = 137,
    Beacon = 138,
    CobblestoneWall = 139,
    Flowerpot = 140,
    Carrots = 141,
    Potatoes = 142,
    WoodenButton = 143,
    Skull = 144,
    Anvil = 145,
    TrappedChest = 146,
    LightWeightedPressurePlate = 147,
    HeavyWeightedPressurePlate = 148,
    UnpoweredComparator = 149,
    PoweredComparator = 150,
    DaylightDetector = 151,
    RedstoneBlock = 152,
    QuartzOre = 153,
    Hopper = 154,
    QuartzBlock = 155,
    QuartzStairs = 156,
    ActivatorRail = 157,
    Dropper = 158,
    StainedHardenedClay = 159,
    StainedGlassPane = 160,
    Leaves2 = 161,
    Log2 = 162,
    AcaciaStairs = 163,
    DarkOakStairs = 164,
    Slime = 165,
    Barrier = 166,
    IronTrapdoor = 167,
    Prismarine = 168,
    SeaLantern = 169,
    HayBlock = 170,
    Carpet = 171,
    HardenedClay = 172,
    CoalBlock = 173,
    PackedIce = 174,
    DoublePlant = 175,
    StandingBanner = 176,
    WallBanner = 177,
    DaylightDetectorInverted = 178,
    RedSandstone = 179,
    RedSandstoneStairs = 180,
    DoubleStoneSlab2 = 181,
    StoneSlab2 = 182,
    SpruceFenceGate = 183,
    BirchFenceGate = 184,
    JungleFenceGate = 185,
    DarkOakFenceGate = 186,
    AcaciaFenceGate = 187,
    SpruceFence = 188,
    BirchFence = 189,
    JungleFence = 190,
    DarkOakFence = 191,
    AcaciaFence = 192,
    SpruceDoor = 193,
    BirchDoor = 194,
    JungleDoor = 195,
    AcaciaDoor = 196,
    DarkOakDoor = 197,
    EndRod = 198,
    ChorusPlant = 199,
    ChorusFlower = 200,
    PurpurBlock = 201,
    PurpurPillar = 202,
    PurpurStairs = 203,
    PurpurDoubleStab = 204,
    PurpurSlab = 205,
    EndBricks = 206,
    GrassPath = 208,
    EndGateaway = 209,
    FrostedIce = 212,
    Magma = 213,
    NetherWartBlock = 214,
    RedNetherBrick = 215,
    BoneBlock = 216,
    Observer = 218,
    PurpleShulkerBox = 229,
    SructureBlock = 255,
    IronShovel = 256,
    IronPickaxe = 257,
    IronAxe = 258,
    FlintAndSteel = 259,
    Apple = 260,
    Bow = 261,
    Arrow = 262,
    Coal = 263,
    Diamond = 264,
    IronIngot = 265,
    GoldIngot = 266,
    IronSword = 267,
    WoodenSword = 268,
    WoodenShovel = 269,
    WoodenPickAxe = 270,
    WoodenAxe = 271,
    StoneSword = 272,
    StoneShovel = 273,
    StonePickAxe = 274,
    StoneAxe = 275,
    DiamondSword = 276,
    DiamondShovel = 277,
    DiamondPickAxe = 278,
    DiamondAxe = 279,
    Stick = 280,
    Bowl = 281,
    MushroomStew = 282,
    GoldenSword = 283,
    GoldenShovel = 284,
    GoldenPickAxe = 285,
    GoldenAxe = 286,
    String = 287,
    Feather = 288,
    Gunpowder = 289,
    WoodenHoe = 290,
    StoneHoe = 291,
    IronHoe = 292,
    DiamondHoe = 293,
    GoldenHoe = 294,
    WheatSeeds = 295,
    Wheat = 296,
    Bread = 297,
    LeatherHelmet = 298,
    LeatherChestplate = 299,
    LeatherLeggings = 300,
    LeatherBoots = 301,
    ChainmailHelmet = 302,
    ChainmaleChestplate = 303,
    ChainmailLeggings = 304,
    ChainmailBoots = 305,
    IronHelmet = 306,
    IronChestplate = 307,
    Ironleggings = 308,
    IronBoots = 309,
    DiamondHelmet = 310,
    DiamondChestplate = 311,
    DiamondLeggings = 312,
    DiamondBoots = 313,
    GoldenHelmet = 314,
    GoldenChestplate = 315,
    GoldenLeggings = 316,
    GoldenBoots = 317,
    Flint = 318,
    Porkchop = 319,
    CookedPorkchop = 320,
    Painting = 321,
    GoldenApple = 322,
    Sign = 323,
    WoodenDoor = 324,
    Bucket = 325,
    WaterBucket = 326,
    LavaBucket = 327,
    Minecart = 328,
    Saddle = 329,
    IronDoor = 330,
    Redstone = 331,
    Snowball = 332,
    Boat = 333,
    Leather = 334,
    MilkBucket = 335,
    Brick = 336,
    ClayBall = 337,
    Reeds = 338,
    Paper = 339,
    Book = 340,
    Slimeball = 341,
    ChestMinecart = 342,
    FurnaceMinecart = 343,
    Egg = 344,
    Compass = 345,
    FishingRod = 346,
    Clock = 347,
    GlowstoneDust = 348,
    Fish = 349,
    CookedFished = 350,
    Dye = 351,
    Bone  = 352,
    Sugar = 353,
    Cake = 354,
    Bed = 355,
    Repeater = 356,
    Cookie = 357,
    FilledMap = 358,
    Shears = 359,
    Melon = 360,
    PumpkinSeeds = 361,
    MelonSeeds = 362,
    Beef = 363,
    CookedBeef = 364,
    Chicken = 365,
    CookedChicken = 366,
    RottenFlesh = 367,
    EnderPearl = 368,
    BlazeRod = 369,
    GhastTear = 370,
    GoldNugget = 371,
    NetherWart = 372,
    Potion = 373,
    GlassBottle = 374,
    SpiderEye = 375,
    FermentedSpiderEye = 376,
    BlazePowder = 377,
    MagmaCream = 378,
    BrewingStand = 379,
    Cauldron = 380,
    EnderEye = 381,
    SpeckledMelon = 382,
    SpawnEgg = 383,
    ExperienceBottle = 384,
    FireCharge = 385,
    WritableBook = 386,
    WrittenBook = 387,
    Emerald = 388,
    ItemFrame = 389,
    FlowerPot = 390,
    Carrot = 391,
    Potato = 392,
    BakedPotato = 393,
    PoisonousPotato = 394,
    Map = 395,
    GoldenCarrot = 396,
    Skull = 397,
    CarrotOnAStick = 398,
    NetherStar = 399,
    PumpkinPie = 400,
    Fireworks = 401,
    Fireworkcharge = 402,
    EnchantedBook = 403,
    Comparator = 404,
    Netherbrick = 405,
    Quartz = 406,
    TntMinecart = 407,
    HopperMinecart = 408,
    PrismarineShard = 409,
    PrismarineCrystals = 410,
    Rabbit = 411,
    CookedRabbit = 412,
    RabbitStew = 413,
    RabbitFoot = 414,
    RabbitHide = 415,
    ArmorStand = 416,
    IronHorseArmor = 417,
    GoldenHorseArmor = 418,
    DiamondHorseArmor = 419,
    Lead = 420,
    NameTag = 421,
    CommandBlockMinecart = 422,
    Mutton = 423,
    CookedMutton = 424,
    Banner = 425,
    EndCrystal = 426,
    SpruceDoor = 427,
    BirchDoor = 428,
    JungleDoor = 429,
    AcaciaDoor = 430,
    DarOakDoor = 431,
    ChorusFruit = 432,
    ChorusFruitPopped = 433,
    Beetroot = 434,
    BeetrootSeeds = 435,
    BeetrootSoup = 436,
    DragonBreath = 437,
    SpectralArrow = 439,
    Shield = 442,
    Elytra = 443,
    Totem = 449,
    ShulkerShell = 450,
    Record13 = 2256,
    RecordCat = 2257,
    RecordBlocks = 2258,
    RecordChirp = 2259,
    RecordFar = 2260,
    RecordMall = 2261,
    RecordMellohi = 2262,
    RecordStal = 2263,
    RecordStrad = 2264,
    RecordWard = 2265,
    Record11 = 2266,
    RecordWait = 2267
}
