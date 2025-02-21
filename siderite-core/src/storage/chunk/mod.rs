pub mod chunk_map;
pub mod section;

use std::io::{Result, Write};

use num_traits::FromPrimitive;

use crate::blocks::BlockType;
use crate::coord::{ChunkCoord, Coord};

use self::section::Section;

/// Width of a chunk
pub const WIDTH: i32 = 16;

/// Height of a chunk column
pub const HEIGHT: i32 = WIDTH * SECTION_COUNT as i32;

/// Area of a chunk
pub const AREA: i32 = WIDTH * WIDTH;

/// Number of sections in a chunk column
pub const SECTION_COUNT: usize = 16;

/// Number of blocks in one section
pub const SECTION_BLOCK_COUNT: usize = (AREA * WIDTH) as usize;

pub trait SerializeChunk {
    fn serialized_size(&self) -> usize;
    fn serialize<W: Write>(&self, w: W) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct ChunkColumn {
    pub sections: [Option<Box<Section>>; SECTION_COUNT],
}

impl ChunkColumn {
    /// Bitmask with 1 for every 16^3 section whose data follows in the compressed data
    pub fn get_primary_bit_mask(&self) -> u16 {
        let mut bit = 0u16;
        for i in 0..SECTION_COUNT {
            if self.sections[i].is_some() {
                bit |= 1 << i;
            }
        }

        bit
    }

    /// Bitmask with 1 for every 16^3 section whose data follows in the compressed data
    pub fn get_num_sections(&self) -> usize {
        self.sections.iter().filter(|x| x.is_some()).count()
    }

    pub fn get_block(&self, rel_pos: Coord<i32>) -> BlockType {
        let (section, index) = ChunkColumn::get_indices_from_rel_pos(rel_pos);

        match &self.sections[section] {
            Some(v) => BlockType::from_u8(v.block_types[index]).unwrap(),
            None => BlockType::Air,
        }
    }

    pub fn set_block(&mut self, rel_pos: Coord<i32>, block_type: BlockType) {
        let (section, index) = ChunkColumn::get_indices_from_rel_pos(rel_pos);

        if self.sections[section].is_none() {
            if block_type == BlockType::Air {
                return;
            }

            self.sections[section] = Some(Box::new(Section {
                block_types: [0; SECTION_BLOCK_COUNT],
                block_metas: [0; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2],
            }));
        }

        match &mut self.sections[section] {
            Some(v) => v.block_types[index] = block_type as u8,
            None => panic!("Dunno"),
        }
    }

    pub fn get_meta(&self, rel_pos: Coord<i32>) -> u8 {
        let (section, index) = ChunkColumn::get_indices_from_rel_pos(rel_pos);

        match &self.sections[section] {
            Some(v) => v.block_metas[index / 2] >> ((index & 1) * 4) & 0x0f,
            None => 0,
        }
    }

    pub fn set_meta(&mut self, rel_pos: Coord<i32>, block_meta: u8) {
        let (section, index) = ChunkColumn::get_indices_from_rel_pos(rel_pos);

        if self.sections[section].is_none() {
            if (block_meta & 0xf) == 0x00 {
                return;
            }

            self.sections[section] = Some(Box::new(Section {
                block_types: [0; SECTION_BLOCK_COUNT],
                block_metas: [0; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2],
            }));
        }

        match &mut self.sections[section] {
            Some(v) => {
                v.block_metas[index / 2] = (v.block_metas[index / 2] & (0xf0 >> ((index & 1) * 4)))
                    | (block_meta & 0x0f) << ((index & 1) * 4)
            }
            None => panic!("Dunno"),
        }
    }

    pub fn get_block_type_meta(&self, rel_pos: Coord<i32>) -> (BlockType, u8) {
        let (section, index) = ChunkColumn::get_indices_from_rel_pos(rel_pos);

        match &self.sections[section] {
            Some(v) => (
                BlockType::from_u8(v.block_types[index]).unwrap(),
                v.block_metas[index / 2] >> ((index & 1) * 4) & 0x0f,
            ),
            None => (BlockType::Air, 0),
        }
    }

    const fn get_indices_from_rel_pos(rel_pos: Coord<i32>) -> (usize, usize) {
        assert!(!Chunk::is_valid_rel_pos(rel_pos));

        (
            (rel_pos.y / WIDTH) as usize,
            (rel_pos.x + rel_pos.z * WIDTH + rel_pos.y * AREA) as usize,
        )
    }
}

pub struct Chunk {
    pub data: ChunkColumn,
    pub biome_map: [u8; AREA as usize],
}

impl Chunk {
    #[inline]
    pub const fn abs_to_rel(pos: Coord<i32>, chunk_coord: ChunkCoord) -> Coord<i32> {
        Coord {
            x: pos.x - chunk_coord.x * WIDTH,
            y: pos.y,
            z: pos.z - chunk_coord.z * WIDTH,
        }
    }

    #[inline]
    pub const fn rel_to_abs(rel_pos: Coord<i32>, chunk_coord: ChunkCoord) -> Coord<i32> {
        Coord {
            x: rel_pos.x + chunk_coord.x * WIDTH,
            y: rel_pos.y,
            z: rel_pos.z + chunk_coord.z * WIDTH,
        }
    }

    #[inline]
    pub const fn is_valid_width(x: i32) -> bool {
        x >= 0 && x < WIDTH
    }

    #[inline]
    pub const fn is_valid_height(y: i32) -> bool {
        y >= 0 && y < HEIGHT
    }

    #[inline]
    pub const fn is_valid_rel_pos(rel_pos: Coord<i32>) -> bool {
        Chunk::is_valid_width(rel_pos.x)
            && Chunk::is_valid_height(rel_pos.y)
            && Chunk::is_valid_width(rel_pos.z)
    }
}
