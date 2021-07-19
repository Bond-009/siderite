use std::collections::HashMap;
use std::sync::RwLock;

use crate::storage::chunk::*;
use crate::storage::chunk::section::Section;

#[derive(Default)]
pub struct ChunkMap {
    // REVIEW: currently we box up the chunks because
    // without they overflow the stack when inserting to the hashmap in debug mode
    chunks: RwLock<HashMap<ChunkCoord, Chunk>>
}

impl ChunkMap {
    pub fn new() -> Self {
        Self {
            chunks: RwLock::new(HashMap::new())
        }
    }

    pub fn do_with_chunk(&self, coord: ChunkCoord, function: impl FnOnce(&Chunk)) {
        let chunks = self.chunks.read().unwrap();

        if let Some(chunk) = chunks.get(&coord) {
            function(chunk);
        }
    }

    pub fn do_with_chunk_mut(&self, coord: ChunkCoord, function: impl FnOnce(&mut Chunk)) {
        let mut chunks = self.chunks.write().unwrap();

        if let Some(chunk) = chunks.get_mut(&coord) {
            function(chunk);
        }
    }

    pub fn touch_chunk(&self, coord: ChunkCoord) {
        {
            let chunks = self.chunks.read().unwrap();
            if chunks.contains_key(&coord) {
                return;
            }
        }

        // TODO: load/generate chunk
        let chunk = Chunk {
            data: ChunkColumn {
                sections: [
                    Some(Box::new(Section {
                        block_types: [3; SECTION_BLOCK_COUNT],
                        block_metas: [0; SECTION_BLOCK_COUNT / 2],
                        block_light: [0; SECTION_BLOCK_COUNT / 2],
                        block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
                    })),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None
                ]},
            biome_map: [1; AREA as usize]
        };

        let mut chunks = self.chunks.write().unwrap();
        chunks.insert(coord, chunk);
    }
}
