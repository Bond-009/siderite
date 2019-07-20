use std::iter::Iterator;
use std::sync::{Arc, mpsc};
use std::thread;

use serde_json as json;
use uuid::Uuid;

use crate::server::Server;

pub struct AuthInfo {
    pub client_id: u32,
    pub username: String,
    pub server_id: Option<String>
}

pub struct Authenticator {
    server: Arc<Server>
}

impl Authenticator {
    pub fn start(server: Arc<Server>) -> mpsc::Sender<AuthInfo> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let authenticator = Authenticator {
                server
            };

            for received in rx.iter() {
                authenticator.handle_request(received);
            }
        });

        tx
    }

    fn handle_request(&self, info: AuthInfo) {
        if self.server.should_authenticate() {
            let res = mojang::auth_with_yggdrasil(&info.username, &info.server_id.unwrap()).unwrap();
            let uuid = Uuid::parse_str(&res.id).unwrap();

            self.server.auth_user(info.client_id, res.name, uuid, res.properties);
            return;
        }

        // TODO: check if UUID is compatible with vanilla
        let uuid = Uuid::new_v3(&Uuid::NAMESPACE_X500, info.username.as_bytes());
        self.server.auth_user(info.client_id, info.username, uuid, json::Value::Null);
    }
}

// TODO: move
pub fn java_hex_digest(mut input: [u8; 20]) -> String {
    const CHARS: &[u8; 16] = b"0123456789abcdef";
    let hex = |byte: u8| { CHARS[byte as usize] };

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
            s.push(hex(b>>4));
        }

        s.push(hex(b&0x0f));
        break;
    }

    for b in iter {
        s.push(hex(b>>4));
        s.push(hex(b&0x0f));
    }

    // Whe know the string is valid UTF-8
    unsafe { String::from_utf8_unchecked(s) }
}

#[inline]
fn twos_compliment(arr: &mut [u8]) {
    let mut carry = true;
    for i in (0..arr.len()).rev() {
        arr[i] = !arr[i];
        if carry {
            carry = arr[i] == 0xFF;
            arr[i] = arr[i].wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::sha::sha1;

    #[test]
    fn notch() {
        let hash = java_hex_digest(sha1(b"Notch"));
        println!("Notch: {}", hash);
        assert_eq!(&hash, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
    }

    #[test]
    fn jeb_() {
        let hash = java_hex_digest(sha1(b"jeb_"));
        println!("jeb_: {}", hash);
        assert_eq!(&hash, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
    }

    #[test]
    fn simon() {
        let hash = java_hex_digest(sha1(b"simon"));
        println!("simon: {}", hash);
        assert_eq!(&hash, "88e16a1019277b15d58faf0541e11910eb756f6");
    }
}
