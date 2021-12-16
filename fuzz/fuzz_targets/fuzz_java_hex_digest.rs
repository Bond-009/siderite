#![no_main]
use libfuzzer_sys::fuzz_target;
use siderite_core::auth;

fuzz_target!(|data: [u8; 20]| {
    auth::java_hex_digest(data);
});
