use std::io::{Result, Write};
use std::mem::size_of;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use mcrw::MCWriteExt;

use crate::storage::chunk::{AREA, SECTION_BLOCK_COUNT, SECTION_COUNT, SerializeChunk, Chunk};
use crate::storage::chunk::section::Section;

impl SerializeChunk for Chunk {
    fn serialized_size(&self) -> usize {
        self.data.get_num_sections() * SECTION_BLOCK_COUNT * 3 + AREA as usize
    }

    fn serialize<W>(&self, mut buf: W) -> Result<()>
        where W: Write {
        buf.write_var_int(self.serialized_size() as i32)?;

        write_block_info(&self.data.sections, &mut buf)?;

        for section in self.data.sections.iter().filter_map(|x| x.as_ref()) {
            buf.write_all(&section.block_light)?;
        }

        for section in self.data.sections.iter().filter_map(|x| x.as_ref()) {
            buf.write_all(&section.block_sky_light)?;
        }

        buf.write_all(&self.biome_map)
    }
}

fn write_block_info<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2") {
            return unsafe { write_block_info_sse2(sections, &mut buf) };
        }
    }

    write_block_info_fallback(sections, &mut buf)
}

fn write_block_info_fallback<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    let mut tmp = [0u8; 4];
    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / 2) {
            let block_type1 = section.block_types[i * 2];
            let block_type2 = section.block_types[i * 2 + 1];
            let block_metas = section.block_metas[i];
            tmp[0] = (block_type1 << 4) | (block_metas & 0x0f);
            tmp[1] = block_type1 >> 4;
            tmp[2] = (block_type2 << 4) | (block_metas >> 4);
            tmp[3] = block_type2 >> 4;
            buf.write_all(&tmp)?;
        }
    }

    Ok(())
}

#[target_feature(enable = "sse2")]
unsafe fn write_block_info_sse2<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const STEP_SIZE: usize = 2 * size_of::<__m128i>() / size_of::<u8>();

    let low_mask = _mm_set1_epi8(0x0f);
    let mut write_buf = [0u8; STEP_SIZE * 2];

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {

            let in_types1 = _mm_load_si128(section.block_types[i * STEP_SIZE..].as_ptr().cast());
            let in_types2 = _mm_load_si128(section.block_types[(i * STEP_SIZE) + (STEP_SIZE / 2)..].as_ptr().cast());

            let in_metas128 = _mm_load_si128(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr().cast());
            let in_metas128_shifted = _mm_srli_epi16::<4>(in_metas128);

            let metas1 = _mm_and_si128(_mm_unpacklo_epi8(in_metas128, in_metas128_shifted), low_mask);
            let metas2 = _mm_and_si128(_mm_unpackhi_epi8(in_metas128, in_metas128_shifted), low_mask);

            let types_shift_right1 = _mm_and_si128(low_mask, _mm_srli_epi16::<4>(in_types1));
            let types_shift_left1 = _mm_andnot_si128(low_mask, _mm_slli_epi16::<4>(in_types1));
            let types_with_metas1 = _mm_or_si128(types_shift_left1, metas1);
            let types_shift_right2 = _mm_and_si128(low_mask, _mm_srli_epi16::<4>(in_types2));
            let types_shift_left2 = _mm_andnot_si128(low_mask, _mm_slli_epi16::<4>(in_types2));
            let types_with_metas2 = _mm_or_si128(types_shift_left2, metas2);

            let first = _mm_unpacklo_epi8(types_with_metas1, types_shift_right1);
            let second = _mm_unpackhi_epi8(types_with_metas1, types_shift_right1);
            let third = _mm_unpacklo_epi8(types_with_metas2, types_shift_right2);
            let fourth = _mm_unpackhi_epi8(types_with_metas2, types_shift_right2);

            _mm_storeu_si128(write_buf.as_mut_ptr().cast(), first);
            _mm_storeu_si128(write_buf[STEP_SIZE / 2..].as_mut_ptr().cast(), second);
            _mm_storeu_si128(write_buf[STEP_SIZE..].as_mut_ptr().cast(), third);
            _mm_storeu_si128(write_buf[STEP_SIZE + (STEP_SIZE / 2)..].as_mut_ptr().cast(), fourth);

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use array_init::array_init;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;

    use super::*;

    use crate::storage::chunk::ChunkColumn;

    impl Arbitrary for Section {
        fn arbitrary(g: &mut Gen) -> Section {
            Section {
                block_types: array_init(|_| u8::arbitrary(g)),
                block_metas: array_init(|_| u8::arbitrary(g)),
                block_light: array_init(|_| u8::arbitrary(g)),
                block_sky_light: array_init(|_| u8::arbitrary(g))
            }
        }
    }

    impl Arbitrary for ChunkColumn {
        fn arbitrary(g: &mut Gen) -> ChunkColumn {
            ChunkColumn {
                sections: array_init(|_| Option::<Box<Section>>::arbitrary(g))
            }
        }
    }

    macro_rules! create_output_buf {
        () => { Vec::with_capacity(SECTION_BLOCK_COUNT * 2) }
    }

    #[quickcheck]
    fn write_block_info_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        write_block_info(&data.sections, &mut buf1).unwrap();
        write_block_info_fallback(&data.sections, &mut buf2).unwrap();
        buf1 == buf2
    }

    #[quickcheck]
    #[cfg(target_feature = "sse2")]
    fn write_block_info_sse2_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        unsafe { write_block_info_sse2(&data.sections, &mut buf1).unwrap(); }
        write_block_info_fallback(&data.sections, &mut buf2).unwrap();
        buf1 == buf2
    }
}
