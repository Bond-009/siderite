use std::io::{Result, Write};
use std::mem::size_of;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use mcrw::MCWriteExt;

use crate::storage::chunk::{AREA, SECTION_BLOCK_COUNT, SerializeChunk, Chunk};
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

pub fn write_block_info<W>(sections: &[Option<Section>; 16], mut buf: W) -> Result<()>
    where W : Write {

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { write_block_info_avx2(sections, &mut buf) };
        }
    }

    write_block_info_fallback(sections, &mut buf)
}

pub fn write_block_info_fallback<W>(sections: &[Option<Section>; 16], mut buf: W) -> Result<()>
    where W : Write {

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..SECTION_BLOCK_COUNT {
            let block_type = section.block_types[i];
            let block_meta = section.block_metas[i / 2] >> ((i & 1) * 4) & 0x0f;
            buf.write_ubyte((block_type << 4) | block_meta)?;
            buf.write_ubyte(block_type >> 4)?;
        }
    }

    Ok(())
}

#[target_feature(enable = "avx2")]
pub unsafe fn write_block_info_avx2<W>(sections: &[Option<Section>; 16], mut buf: W) -> Result<()>
    where W : Write {

    const STEP_SIZE: usize = size_of::<__m256i>() / size_of::<u8>();

    let low_mask = _mm256_set1_epi8(0x0f);
    let mut write_buf = [0u8; STEP_SIZE * 2];

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {
            let in_types = _mm256_load_si256(section.block_types[i * STEP_SIZE..].as_ptr() as *const __m256i);
            let in_metas128 = _mm_load_si128(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr() as *const __m128i);
            let in_metas128_shifted = _mm_srli_epi16(in_metas128, 4);

            let metas_low = _mm_unpacklo_epi8(in_metas128, in_metas128_shifted);
            let metas_high = _mm_unpackhi_epi8(in_metas128, in_metas128_shifted);
            let mut metas = _mm256_set_m128i(metas_high, metas_low);
            metas = _mm256_and_si256(metas, low_mask);

            let types_shift_left = _mm256_andnot_si256(low_mask, _mm256_slli_epi16(in_types, 4));
            let types_shift_right = _mm256_and_si256(low_mask, _mm256_srli_epi16(in_types, 4));
            let types_with_metas = _mm256_or_si256(types_shift_left, metas);
            let first = _mm256_unpacklo_epi8(types_with_metas, types_shift_right);
            let second = _mm256_unpackhi_epi8(types_with_metas, types_shift_right);
            _mm256_storeu2_m128i(write_buf[STEP_SIZE..].as_mut_ptr() as *mut __m128i, write_buf.as_mut_ptr() as *mut __m128i, first);
            _mm256_storeu2_m128i(write_buf[STEP_SIZE + STEP_SIZE / 2..].as_mut_ptr() as *mut __m128i, write_buf[STEP_SIZE / 2..].as_mut_ptr() as *mut __m128i, second);
            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}
