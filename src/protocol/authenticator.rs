use std::iter::Iterator;
use std::sync::{Arc, mpsc};
use std::thread;

use hex;
use mojang::auth_with_yggdrasil;
use serde_json as json;
use uuid::Uuid;

use crate::server::Server;

pub struct AuthInfo {
    pub client_id: i32,
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
                server: server
            };

            for received in rx.iter() {
                authenticator.handle_request(received);
            }
        });

        tx
    }

    fn handle_request(&self, info: AuthInfo) {
        if !self.server.authenticate {
            // TODO: check if UUID is compatible with vanilla
            let uuid = Uuid::new_v3(&Uuid::NAMESPACE_X500, info.username.as_bytes());
            self.server.auth_user(info.client_id, info.username, uuid, json::Value::Null);
            return;
        }

        let res = auth_with_yggdrasil(&info.username, &info.server_id.unwrap()).unwrap();
        let uuid = Uuid::parse_str(&res.id).unwrap();
        
        self.server.auth_user(info.client_id, res.name, uuid, res.properties);
    }
}

// TODO: move
pub fn java_hex_digest(mut input: [u8; 20]) -> String {
    let negative = (input[0] & 0x80) == 0x80;
    if negative {
        twos_compliment(&mut input);
    }
    let mut digest = hex::encode(input);
    digest = digest.trim_start_matches('0').to_owned();
    if negative {
        digest.insert(0, '-');
    }
    digest
}

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
