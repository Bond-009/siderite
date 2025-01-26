use std::io::{Result, Write};
use std::mem::size_of;

#[cfg(target_arch = "aarch64")]
use std::arch::is_aarch64_feature_detected;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;
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
        if is_x86_feature_detected!("avx2") {
            return unsafe { write_block_info_avx2(sections, &mut buf) };
        }

        if is_x86_feature_detected!("sse2") {
            return unsafe { write_block_info_sse2(sections, &mut buf) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if is_aarch64_feature_detected!("neon") {
            return unsafe { write_block_info_neon(sections, &mut buf) };
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

#[repr(C, align(16))]
struct Align16<const N: usize>([u8; N]);

impl<const N: usize> Default for Align16<N> {
    fn default() -> Self {
        Self([0u8; N])
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse2")]
unsafe fn write_block_info_sse2<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const VECTOR_SIZE: usize = size_of::<__m128i>();
    const STEP_SIZE: usize = 2 * VECTOR_SIZE;
    const BUF_SIZE: usize = 2 * STEP_SIZE;

    let low_mask = _mm_set1_epi8(0x0f);

    let mut write_buf = Align16::<BUF_SIZE>::default().0;

    // Validate that buffer is 16-byte aligned
    debug_assert_eq!(write_buf.as_ptr() as usize & 15, 0);

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {

            let in_types1 = _mm_load_si128(section.block_types[i * STEP_SIZE..].as_ptr().cast());
            let in_types2 = _mm_load_si128(section.block_types[i * STEP_SIZE + VECTOR_SIZE..].as_ptr().cast());

            let in_metas = _mm_load_si128(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr().cast());
            let in_metas_shifted = _mm_srli_epi16::<4>(in_metas);

            let metas1 = _mm_and_si128(_mm_unpacklo_epi8(in_metas, in_metas_shifted), low_mask);
            let metas2 = _mm_and_si128(_mm_unpackhi_epi8(in_metas, in_metas_shifted), low_mask);

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

            _mm_store_si128(write_buf.as_mut_ptr().cast(), first);
            _mm_store_si128(write_buf[VECTOR_SIZE..].as_mut_ptr().cast(), second);
            _mm_store_si128(write_buf[2 * VECTOR_SIZE..].as_mut_ptr().cast(), third);
            _mm_store_si128(write_buf[3 * VECTOR_SIZE..].as_mut_ptr().cast(), fourth);

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[repr(C, align(32))]
struct Align32<const N: usize>([u8; N]);

impl<const N: usize> Default for Align32<N> {
    fn default() -> Self {
        Self([0u8; N])
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn write_block_info_avx2<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const VECTOR_SIZE: usize = size_of::<__m256i>();
    const STEP_SIZE: usize = 2 * VECTOR_SIZE;
    const BUF_SIZE: usize = 2 * STEP_SIZE;

    let low_mask = _mm256_set1_epi8(0x0f);

    let mut write_buf = Align32::<BUF_SIZE>::default().0;

    // Validate that buffer is 32-byte aligned
    debug_assert_eq!(write_buf.as_ptr() as usize & 31, 0);

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {

            let in_types1 = _mm256_load_si256(section.block_types[i * STEP_SIZE..].as_ptr().cast());
            let in_types2 = _mm256_load_si256(section.block_types[i * STEP_SIZE + VECTOR_SIZE..].as_ptr().cast());

            let in_metas = _mm256_permute4x64_epi64(_mm256_load_si256(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr().cast()), 0b11011000);
            let in_metas_shifted = _mm256_srli_epi16::<4>(in_metas);

            let metas1 = _mm256_and_si256(_mm256_unpacklo_epi8(in_metas, in_metas_shifted), low_mask);
            let metas2 = _mm256_and_si256(_mm256_unpackhi_epi8(in_metas, in_metas_shifted), low_mask);

            let types_shift_right1 = _mm256_and_si256(low_mask, _mm256_srli_epi16::<4>(in_types1));
            let types_shift_left1 = _mm256_andnot_si256(low_mask, _mm256_slli_epi16::<4>(in_types1));
            let types_with_metas1 = _mm256_or_si256(types_shift_left1, metas1);
            let types_shift_right2 = _mm256_and_si256(low_mask, _mm256_srli_epi16::<4>(in_types2));
            let types_shift_left2 = _mm256_andnot_si256(low_mask, _mm256_slli_epi16::<4>(in_types2));
            let types_with_metas2 = _mm256_or_si256(types_shift_left2, metas2);

            let first = _mm256_unpacklo_epi8(types_with_metas1, types_shift_right1);
            let second = _mm256_unpackhi_epi8(types_with_metas1, types_shift_right1);
            let third = _mm256_unpacklo_epi8(types_with_metas2, types_shift_right2);
            let fourth = _mm256_unpackhi_epi8(types_with_metas2, types_shift_right2);

            _mm256_store_si256(write_buf.as_mut_ptr().cast(), _mm256_permute2x128_si256(first, second, 0x20));
            _mm256_store_si256(write_buf[VECTOR_SIZE..].as_mut_ptr().cast(), _mm256_permute2x128_si256(first, second, 0x31));
            _mm256_store_si256(write_buf[2 * VECTOR_SIZE..].as_mut_ptr().cast(), _mm256_permute2x128_si256(third, fourth, 0x20));
            _mm256_store_si256(write_buf[3 * VECTOR_SIZE..].as_mut_ptr().cast(), _mm256_permute2x128_si256(third, fourth, 0x31));

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn write_block_info_neon<W>(sections: &[Option<Box<Section>>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const VECTOR_SIZE: usize = size_of::<uint8x16_t>();
    const STEP_SIZE: usize = 2 * VECTOR_SIZE;
    const BUF_SIZE: usize = 2 * STEP_SIZE;

    let low_mask = vdupq_n_u8(0x0f);
    let mut write_buf = Align16::<BUF_SIZE>::default().0;

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {

            let in_types1 = vld1q_u8(section.block_types[i * STEP_SIZE..].as_ptr());
            let in_types2 = vld1q_u8(section.block_types[(i * STEP_SIZE) + (STEP_SIZE / 2)..].as_ptr());

            let in_metas128 = vld1q_u8(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr());
            let in_metas128_shifted = vshrq_n_u8(in_metas128, 4);
            let in_metas128_low = vandq_u8(in_metas128, low_mask);

            let metas = vzipq_u8(in_metas128_low, in_metas128_shifted);

            let types_shift_right1 = vshrq_n_u8(in_types1, 4);
            let types_shift_left1 = vshlq_n_u8(in_types1, 4);
            let types_with_metas1 = vorrq_u8(types_shift_left1, metas.0);
            let types_shift_right2 = vshrq_n_u8(in_types2, 4);
            let types_shift_left2 = vshlq_n_u8(in_types2, 4);
            let types_with_metas2 = vorrq_u8(types_shift_left2, metas.1);

            vst2q_u8(write_buf.as_mut_ptr(), uint8x16x2_t { 0: types_with_metas1, 1: types_shift_right1});
            vst2q_u8(write_buf[STEP_SIZE..].as_mut_ptr(), uint8x16x2_t { 0: types_with_metas2, 1: types_shift_right2});

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate test;

    use std::array;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use test::Bencher;
    use test::bench::black_box;

    use super::*;

    use crate::storage::chunk::ChunkColumn;

    impl Arbitrary for Section {
        fn arbitrary(g: &mut Gen) -> Section {
            Section {
                block_types: array::from_fn(|_| u8::arbitrary(g)),
                block_metas: array::from_fn(|_| u8::arbitrary(g)),
                block_light: array::from_fn(|_| u8::arbitrary(g)),
                block_sky_light: array::from_fn(|_| u8::arbitrary(g))
            }
        }
    }

    impl Arbitrary for ChunkColumn {
        fn arbitrary(g: &mut Gen) -> ChunkColumn {
            ChunkColumn {
                sections: array::from_fn(|_| Option::<Box<Section>>::arbitrary(g))
            }
        }
    }

    macro_rules! create_output_buf {
        () => { [0u8; SECTION_COUNT * SECTION_BLOCK_COUNT * 2] }
    }

    #[quickcheck]
    fn write_block_info_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        write_block_info(&data.sections, buf1.as_mut_slice()).unwrap();
        write_block_info_fallback(&data.sections, buf2.as_mut_slice()).unwrap();
        buf1 == buf2
    }

    #[quickcheck]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[cfg(target_feature = "sse2")]
    fn write_block_info_sse2_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        unsafe { write_block_info_sse2(&data.sections, buf1.as_mut_slice()).unwrap(); }
        write_block_info_fallback(&data.sections, buf2.as_mut_slice()).unwrap();
        buf1 == buf2
    }

    #[quickcheck]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[cfg(target_feature = "avx2")]
    fn write_block_info_avx2_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        unsafe { write_block_info_avx2(&data.sections, buf1.as_mut_slice()).unwrap(); }
        write_block_info_fallback(&data.sections, buf2.as_mut_slice()).unwrap();
        buf1 == buf2
    }

    #[quickcheck]
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    #[cfg(target_feature = "neon")]
    fn write_block_info_neon_matches_fallback(data: ChunkColumn) -> bool {
        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        unsafe { write_block_info_neon(&data.sections, buf1.as_mut_slice()).unwrap(); }
        write_block_info_fallback(&data.sections, buf2.as_mut_slice()).unwrap();
        &buf1 == &buf2
    }

    #[test]
    #[cfg(target_feature = "neon")]
    fn write_block_info_neon_test() {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
        Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        let mut buf2 = create_output_buf!();
        unsafe { write_block_info_neon(&data, buf1.as_mut_slice()).unwrap(); }
        write_block_info_fallback(&data, buf2.as_mut_slice()).unwrap();
        assert_eq!(&buf1, &buf2);
    }

    #[bench]
    #[cfg(target_feature = "neon")]
    fn write_block_info_neon_bench(b: &mut Bencher) {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        b.iter(|| {
            unsafe { write_block_info_neon(black_box(&data), buf1.as_mut_slice()).unwrap(); }
        });
    }


    #[bench]
    fn write_block_info_bench(b: &mut Bencher) {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        b.iter(|| {
            write_block_info(black_box(&data), buf1.as_mut_slice()).unwrap();
        });
    }

    #[bench]
    #[cfg(target_feature = "sse2")]
    fn write_block_info_avx2_bench(b: &mut Bencher) {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        b.iter(|| {
            unsafe { write_block_info_avx2(black_box(&data), buf1.as_mut_slice()).unwrap(); }
        });
    }

    #[bench]
    #[cfg(target_feature = "sse2")]
    fn write_block_info_sse2_bench(b: &mut Bencher) {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        b.iter(|| {
            unsafe { write_block_info_sse2(black_box(&data), buf1.as_mut_slice()).unwrap(); }
        });
    }

    #[bench]
    fn write_block_info_fallback_bench(b: &mut Bencher) {
        let data = [
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            })),
            Some(Box::new(Section {
                block_types: [3; SECTION_BLOCK_COUNT],
                block_metas: [1; SECTION_BLOCK_COUNT / 2],
                block_light: [0; SECTION_BLOCK_COUNT / 2],
                block_sky_light: [0xff; SECTION_BLOCK_COUNT / 2]
            }))
        ];

        let mut buf1 = create_output_buf!();
        b.iter(|| {
            write_block_info_fallback(black_box(&data), buf1.as_mut_slice()).unwrap();
        });
    }
}
