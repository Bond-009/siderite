use std::sync::{Arc, mpsc};
use std::{thread};

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
            for received in rx {
                Authenticator::handle_request(server.clone(), received);
            }
        });

        tx
    }

    fn handle_request(_server: Arc<Server>, _info: AuthInfo) {
        // TODO
    }
}
