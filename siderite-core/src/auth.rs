use std::iter::Iterator;
use std::result;

use async_trait::async_trait;
use json::Value;
use openssl::error::ErrorStack;
use openssl::hash::{Hasher, MessageDigest};
use serde_json as json;
use uuid::Uuid;

pub type Result = result::Result<AuthResponse, Error>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    NoServerId,
    Failed
}

pub struct AuthResponse {
    pub client_id: u32,
    pub username: String,
    pub uuid: Uuid,
    pub properties: Value
}

pub struct AuthInfo {
    pub client_id: u32,
    pub server_id: Option<String>,
    pub username: String
}

#[async_trait]
pub trait Authenticator : Send + Sync {
    async fn authenticate(&self, info: AuthInfo) -> Result;
}

pub struct OfflineAuthenticator;

#[async_trait]
impl Authenticator for OfflineAuthenticator {
    async fn authenticate(&self, info: AuthInfo) -> Result {
        let uuid = generate_offline_uuid(&info.username).map_err(|_| Error::Failed)?;
        Ok(AuthResponse {
            client_id: info.client_id,
            username: info.username,
            uuid,
            properties: json::Value::Null
        })
    }
}

///```
/// use uuid::Uuid;
/// use siderite_core::auth;
///
/// let uuid = auth::generate_offline_uuid("Bond_009").unwrap();
/// assert_eq!(uuid, Uuid::parse_str("299ced23-a208-3ef3-99e3-206968219434").unwrap());
///```
pub fn generate_offline_uuid(username: &str) -> result::Result<Uuid, ErrorStack> {
    let mut h = Hasher::new(MessageDigest::md5())?;
    h.update(b"OfflinePlayer:")?;
    h.update(username.as_bytes())?;
    let digest = h.finish()?;

    let mut b = [0u8; 16];
    b.copy_from_slice(&digest);

    Ok(uuid::Builder::from_md5_bytes(b).into_uuid())
}

// TODO: move
///```
/// use openssl::sha::sha1;
/// use siderite_core::auth;
///
/// let hex = auth::java_hex_digest(sha1(b"Notch"));
/// assert_eq!(&hex, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
///```
pub fn java_hex_digest(mut input: [u8; 20]) -> String {
    const fn hex(byte: u8) -> u8 {
        b"0123456789abcdef"[byte as usize]
    }

    // The max size is 2 * the length of the input array + 1 for the possible '-' sign
    let mut s = Vec::with_capacity(20 * 2 + 1);

    if (input[0] & 0x80) == 0x80 {
        twos_compliment(&mut input);
        s.push(b'-');
    }

    let mut iter = input.iter();
    // Ignore the first '0's
    for b in &mut iter {
        if *b == 0 {
            continue;
        }

        if *b >= 16 {
            s.push(hex(b >> 4));
        }

        s.push(hex(b & 0x0f));
        break;
    }

    for b in iter {
        s.push(hex(b >> 4));
        s.push(hex(b & 0x0f));
    }

    // We know the string is valid UTF-8
    unsafe { String::from_utf8_unchecked(s) }
}

#[inline]
fn twos_compliment(arr: &mut [u8; 20]) {
    let mut carry = true;
    for x in arr.iter_mut().rev() {
        (*x, carry) = (!*x).overflowing_add(carry as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::sha::sha1;

    macro_rules! java_hex_digest_test {
        ($func_name:ident, $arg:expr, $expected:expr) => {
            #[test]
            fn $func_name() {
                let hash = java_hex_digest(sha1($arg));
                assert_eq!(&hash, $expected);
            }
        };
    }

    java_hex_digest_test!(notch, b"Notch", "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
    java_hex_digest_test!(jeb_, b"jeb_", "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
    java_hex_digest_test!(simon, b"simon", "88e16a1019277b15d58faf0541e11910eb756f6");
}
