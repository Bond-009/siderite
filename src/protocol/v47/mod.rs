use std::io::Write;

use mcrw::MCWriteExt;

use crate::storage::chunk::*;

impl SerializeChunk for Chunk {
    fn serialized_size(&self) -> usize {
        self.data.get_num_sections() * SECTION_BLOCK_COUNT * 3 + AREA as usize
    }

    fn serialize<W>(&self, mut buf: W)
        where W: Write {
        buf.write_var_int(self.serialized_size() as i32).unwrap();

        for section in self.data.sections.iter() {
            match section {
                Some(v) => {
                    for i in 0..SECTION_BLOCK_COUNT {
                        let block_type = v.block_types[i];
                        let block_meta = v.block_metas[i / 2] >> ((i & 1) * 4) & 0x0f;
                        buf.write_ubyte((block_type << 4) | block_meta).unwrap();
                        buf.write_ubyte(block_type >> 4).unwrap();
                    }
                },
                None => ()
            }
        }

        for section in self.data.sections.iter() {
            match section {
                Some(v) => {
                    buf.write_all(&v.block_light).unwrap();
                },
                None => ()
            }
        }

        for section in self.data.sections.iter() {
            match section {
                Some(v) => {
                    buf.write_all(&v.block_sky_light).unwrap();
                },
                None => ()
            }
        }

        buf.write_all(&self.biome_map).unwrap();
    }
}
