pub mod section;
pub mod chunk_map;

use std::io::Write;

use self::section::Section;

pub const HEIGTH: i32 = 265;
pub const WIDTH: i32 = 16;
pub const AREA: i32 = WIDTH * WIDTH;
pub const SECTION_COUNT: usize = (HEIGTH / WIDTH) as usize;
pub const SECTION_BLOCK_COUNT: usize = (AREA * WIDTH) as usize;

pub trait SerializeChunk {
    fn serialized_size(&self) -> usize;
    fn serialize<W: Write>(&self, buf: W);
}

pub struct ChunkColumn {
    pub sections: [Option<Section>; SECTION_COUNT]
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
}

pub struct Chunk {
    pub data: ChunkColumn
}
