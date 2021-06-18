use std::io::{Result, Write};
use std::mem::size_of;

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use mcrw::MCWriteExt;

use crate::storage::chunk::{AREA, SECTION_COUNT, SECTION_BLOCK_COUNT, SerializeChunk, Chunk};
use crate::storage::chunk::section::Section;

impl SerializeChunk for Chunk {
    fn serialized_size(&self) -> usize {
        self.data.get_num_sections() * size_of::<Section>() + AREA
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

fn write_block_info<W>(sections: &[Option<Section>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { write_block_info_avx2(sections, &mut buf) };
        }
        else if is_x86_feature_detected!("sse2") {
            return unsafe { write_block_info_sse2(sections, &mut buf) };
        }
    }

    write_block_info_fallback(sections, &mut buf)
}

fn write_block_info_fallback<W>(sections: &[Option<Section>; SECTION_COUNT], mut buf: W) -> Result<()>
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
unsafe fn write_block_info_sse2<W>(sections: &[Option<Section>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const STEP_SIZE: usize = 2 * size_of::<__m128i>() / size_of::<u8>();

    let low_mask = _mm_set1_epi8(0x0f);
    let mut write_buf = [0u8; STEP_SIZE * 2];

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {

            let in_types1 = _mm_load_si128(section.block_types[i * STEP_SIZE..].as_ptr() as *const __m128i);
            let in_types2 = _mm_load_si128(section.block_types[(i * STEP_SIZE) + (STEP_SIZE / 2)..].as_ptr() as *const __m128i);

            let in_metas128 = _mm_load_si128(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr() as *const __m128i);
            let in_metas128_shifted = _mm_srli_epi16(in_metas128, 4);

            let metas1 = _mm_and_si128(_mm_unpacklo_epi8(in_metas128, in_metas128_shifted), low_mask);
            let metas2 = _mm_and_si128(_mm_unpackhi_epi8(in_metas128, in_metas128_shifted), low_mask);

            let types_shift_right1 = _mm_and_si128(low_mask, _mm_srli_epi16(in_types1, 4));
            let types_shift_left1 = _mm_andnot_si128(low_mask, _mm_slli_epi16(in_types1, 4));
            let types_with_metas1 = _mm_or_si128(types_shift_left1, metas1);
            let types_shift_right2 = _mm_and_si128(low_mask, _mm_srli_epi16(in_types2, 4));
            let types_shift_left2 = _mm_andnot_si128(low_mask, _mm_slli_epi16(in_types2, 4));
            let types_with_metas2 = _mm_or_si128(types_shift_left2, metas2);

            let first = _mm_unpacklo_epi8(types_with_metas1, types_shift_right1);
            let second = _mm_unpackhi_epi8(types_with_metas1, types_shift_right1);
            let third = _mm_unpacklo_epi8(types_with_metas2, types_shift_right2);
            let fourth = _mm_unpackhi_epi8(types_with_metas2, types_shift_right2);

            _mm_storeu_si128(write_buf.as_mut_ptr() as *mut __m128i, first);
            _mm_storeu_si128(write_buf[STEP_SIZE / 2..].as_mut_ptr() as *mut __m128i, second);
            _mm_storeu_si128(write_buf[STEP_SIZE..].as_mut_ptr() as *mut __m128i, third);
            _mm_storeu_si128(write_buf[STEP_SIZE + (STEP_SIZE / 2)..].as_mut_ptr() as *mut __m128i, fourth);

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[target_feature(enable = "avx2")]
unsafe fn write_block_info_avx2<W>(sections: &[Option<Section>; SECTION_COUNT], mut buf: W) -> Result<()>
    where W : Write {

    const STEP_SIZE: usize = 2 * size_of::<__m256i>() / size_of::<u8>();

    let low_mask = _mm256_set1_epi8(0x0f);
    let mut write_buf = [0u8; STEP_SIZE * 2];

    for section in sections.iter().filter_map(|x| x.as_ref()) {
        for i in 0..(SECTION_BLOCK_COUNT / STEP_SIZE) {
            let in_types1 = _mm256_load_si256(section.block_types[i * STEP_SIZE..].as_ptr() as *const __m256i);
            let in_types2 = _mm256_load_si256(section.block_types[(i * STEP_SIZE) + (STEP_SIZE / 2)..].as_ptr() as *const __m256i);

            let in_metas256 = _mm256_load_si256(section.block_metas[i * (STEP_SIZE / 2)..].as_ptr() as *const __m256i);
            let in_metas256_shifted = _mm256_srli_epi16(in_metas256, 4);

            let metas_low = _mm256_and_si256(_mm256_unpacklo_epi8(in_metas256, in_metas256_shifted), low_mask);
            let metas_high = _mm256_and_si256(_mm256_unpackhi_epi8(in_metas256, in_metas256_shifted), low_mask);

            let metas1 = _mm256_permute2x128_si256(metas_low, metas_high, 0x20);
            let metas2 = _mm256_permute2x128_si256(metas_low, metas_high, 0x31);

            let types_shift_left1 = _mm256_andnot_si256(low_mask, _mm256_slli_epi16(in_types1, 4));
            let types_shift_right1 = _mm256_and_si256(low_mask, _mm256_srli_epi16(in_types1, 4));
            let types_with_metas1 = _mm256_or_si256(types_shift_left1, metas1);
            let types_shift_left2 = _mm256_andnot_si256(low_mask, _mm256_slli_epi16(in_types2, 4));
            let types_shift_right2 = _mm256_and_si256(low_mask, _mm256_srli_epi16(in_types2, 4));
            let types_with_metas2 = _mm256_or_si256(types_shift_left2, metas2);

            let first = _mm256_unpacklo_epi8(types_with_metas1, types_shift_right1);
            let second = _mm256_unpackhi_epi8(types_with_metas1, types_shift_right1);
            let third = _mm256_unpacklo_epi8(types_with_metas2, types_shift_right2);
            let fourth = _mm256_unpackhi_epi8(types_with_metas2, types_shift_right2);

            _mm256_storeu2_m128i(write_buf[(STEP_SIZE / 2)..].as_mut_ptr() as *mut __m128i, write_buf.as_mut_ptr() as *mut __m128i, first);
            _mm256_storeu2_m128i(write_buf[(STEP_SIZE / 4) * 3..].as_mut_ptr() as *mut __m128i, write_buf[(STEP_SIZE / 4)..].as_mut_ptr() as *mut __m128i, second);
            _mm256_storeu2_m128i(write_buf[(STEP_SIZE / 4) * 6..].as_mut_ptr() as *mut __m128i, write_buf[STEP_SIZE..].as_mut_ptr() as *mut __m128i, third);
            _mm256_storeu2_m128i(write_buf[(STEP_SIZE / 4) * 7..].as_mut_ptr() as *mut __m128i, write_buf[(STEP_SIZE / 4) * 5..].as_mut_ptr() as *mut __m128i, fourth);

            buf.write_all(&write_buf)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate test;
    use test::Bencher;
    use test::black_box;

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
                sections: array_init(|_| Option::<Section>::arbitrary(g))
            }
        }
    }

    macro_rules! create_test_data {
        () => {
            [
                Some(Section {
                    block_types: [3; SECTION_BLOCK_COUNT],
                    block_metas: [1; SECTION_BLOCK_COUNT / 2],
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
                None,
            ];
        };
    }

    macro_rules! test_write_block_info_fn {
        ($f:ident) => {
            let data = create_test_data!();
            let mut buf1 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
            let mut buf2 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
            $f(&data, &mut buf1).unwrap();
            write_block_info_fallback(&data, &mut buf2).unwrap();
            assert_eq!(&buf1, &buf2);
        };
    }

    macro_rules! bench_write_block_info_fn {
        ($f:ident, $b:ident) => {
            let data = create_test_data!();
            let mut buf = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
            $b.iter(|| {
                $f(black_box(&data), black_box(&mut buf)).unwrap();
                buf.clear();
            });
        };
    }

    #[test]
    fn test_write_block_info() {
        test_write_block_info_fn!(write_block_info);
    }

    #[test]
    #[cfg(target_feature = "sse2")]
    fn test_write_block_info_sse2() {
        unsafe { test_write_block_info_fn!(write_block_info_sse2); }
    }

    #[test]
    #[cfg(target_feature = "avx2")]
    fn test_write_block_info_avx2() {
        unsafe { test_write_block_info_fn!(write_block_info_avx2); }
    }

    #[quickcheck]
    fn write_block_info_matches_fallback(data: Box<ChunkColumn>) -> bool {
        let mut buf1 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        let mut buf2 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        write_block_info(&data.sections, &mut buf1).unwrap();
        write_block_info_fallback(&data.sections, &mut buf2).unwrap();
        &buf1 == &buf2
    }

    #[quickcheck]
    #[cfg(target_feature = "sse2")]
    fn write_block_info_sse2_matches_fallback(data: Box<ChunkColumn>) -> bool {
        let mut buf1 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        let mut buf2 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        unsafe { write_block_info_sse2(&data.sections, &mut buf1).unwrap(); }
        write_block_info_fallback(&data.sections, &mut buf2).unwrap();
        &buf1 == &buf2
    }

    #[quickcheck]
    #[cfg(target_feature = "avx2")]
    fn write_block_info_avx2_matches_fallback(data: Box<ChunkColumn>) -> bool {
        let mut buf1 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        let mut buf2 = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
        unsafe { write_block_info_avx2(&data.sections, &mut buf1).unwrap(); }
        write_block_info_fallback(&data.sections, &mut buf2).unwrap();
        &buf1 == &buf2
    }

    #[bench]
    fn bench_write_block_info(b: &mut Bencher) {
        bench_write_block_info_fn!(write_block_info, b);
    }

    #[bench]
    fn bench_write_block_info_fallback(b: &mut Bencher) {
        bench_write_block_info_fn!(write_block_info_fallback, b);
    }

    #[bench]
    #[cfg(target_feature = "sse2")]
    fn bench_write_block_info_sse2(b: &mut Bencher) {
        unsafe { bench_write_block_info_fn!(write_block_info_sse2, b); };
    }

    #[bench]
    #[cfg(target_feature = "avx2")]
    fn bench_write_block_info_avx2(b: &mut Bencher) {
        unsafe { bench_write_block_info_fn!(write_block_info_avx2, b); };
    }
}
