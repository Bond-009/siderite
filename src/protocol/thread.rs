use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, SystemTime};

use crate::protocol::Protocol;

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_millis(1000);

pub struct ProtocolThread {
    rx: Receiver<Protocol>,
    prots: Vec<Protocol>,
    last_keep_alive: SystemTime
}

impl ProtocolThread {
    pub fn start() -> mpsc::Sender<Protocol> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut thread = ProtocolThread {
                rx,
                prots: Vec::new(),
                last_keep_alive: SystemTime::now()
            };

            loop {
                thread.tick();
                thread::sleep(Duration::from_millis(20));
            }
        });

        tx
    }

    fn tick(&mut self) {
        self.prots.retain(|x| !x.is_disconnected()); // TODO: destroy clients

        for prot in self.rx.try_iter() {
            self.prots.push(prot);
        }

        let send_keep_alive = self.last_keep_alive.elapsed().unwrap() >= KEEP_ALIVE_INTERVAL;
        let millis = if send_keep_alive {
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i32
        } else {
            0
        };

        for prot in self.prots.iter_mut() {
            if prot.is_disconnected() {
                // We'll handle it next tick
                continue;
            }

            prot.process_data();
            if send_keep_alive {
                prot.keep_alive(millis);
            }

            prot.handle_out_packets();
        }
    }
}
