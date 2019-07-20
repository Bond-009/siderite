use super::*;

pub struct Section {
    pub block_types: [u8; SECTION_BLOCK_COUNT],
    pub block_metas: [u8; SECTION_BLOCK_COUNT / 2],
    pub block_light: [u8; SECTION_BLOCK_COUNT / 2],
    pub block_sky_light: [u8; SECTION_BLOCK_COUNT / 2],
}
