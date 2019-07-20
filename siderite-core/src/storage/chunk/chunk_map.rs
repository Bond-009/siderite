use std::collections::HashMap;
use std::sync::RwLock;

use crate::storage::chunk::*;
use crate::storage::chunk::section::Section;

pub struct ChunkMap {
    chunks: RwLock<HashMap<ChunkCoord, Chunk>>
}

impl ChunkMap {
    pub fn default() -> ChunkMap {
        ChunkMap {
            chunks: RwLock::new(HashMap::new())
        }
    }

    pub fn do_with_chunk(&self, coord: ChunkCoord, function: &mut dyn FnMut(&Chunk)) {
        let chunks = self.chunks.read().unwrap();

        if let Some(chunk) = chunks.get(&coord) {
            function(chunk);
        }
    }

    pub fn do_with_chunk_mut(&self, coord: ChunkCoord, function: &mut dyn FnMut(&mut Chunk)) {
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
                    Some(Section {
                        block_types: [3; SECTION_BLOCK_COUNT],
                        block_metas: [0; SECTION_BLOCK_COUNT / 2],
                        block_light: [0; SECTION_BLOCK_COUNT / 2],
                        block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
                    }),
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
