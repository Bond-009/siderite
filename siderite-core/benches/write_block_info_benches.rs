#![feature(test)]
extern crate test;
use test::Bencher;
use test::black_box;

use siderite_core::storage::chunk::section::*;
use siderite_core::protocol::v47;

#[macro_export]
macro_rules! create_test_data {
    () => {
        [
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
        ];
    };
}

#[bench]
fn bench_write_block_info(b: &mut Bencher) {
    let data = create_test_data!();
    let mut buf = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
    b.iter(|| v47::write_block_info(black_box(&data), black_box(&mut buf)));
}

#[bench]
fn bench_write_block_info_fallback(b: &mut Bencher) {
    let data = create_test_data!();
    let mut buf = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
    b.iter(|| v47::write_block_info_fallback(black_box(&data), black_box(&mut buf)));
}

#[bench]
fn bench_write_block_info_avx2(b: &mut Bencher) {
    let data = create_test_data!();
    let mut buf = Vec::with_capacity(SECTION_BLOCK_COUNT * 2);
    b.iter(|| unsafe { v47::write_block_info_avx2(black_box(&data), black_box(&mut buf)) } );
}
