use std::sync::{Arc, mpsc};
use std::{thread};

use serde_json as json;
use uuid::Uuid;

use server::Server;

pub struct AuthInfo {
    pub client_id: i32,
    pub username: String
}

pub struct Authenticator {
}

impl Authenticator {
    pub fn start(server: Arc<Server>) -> mpsc::Sender<AuthInfo> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            for received in rx.iter() {
                Authenticator::handle_request(server.clone(), received);
            }
        });

        tx
    }

    fn handle_request(server: Arc<Server>, info: AuthInfo) {
        if !server.authenticate {
            // TODO: check if UUID is compatible with vanilla
            let uuid = Uuid::new_v3(&Uuid::NAMESPACE_X500, info.username.as_bytes());
            server.auth_user(info.client_id, info.username, uuid, json::Value::Null);
        }

        // TODO: auth
    }
}
