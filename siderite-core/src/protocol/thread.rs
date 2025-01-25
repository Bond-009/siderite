use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};

use crate::TICK_DURATION;
use crate::protocol::Protocol;

const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(2);

pub struct ProtocolThread {
    rx: Receiver<Protocol>,
    prots: Vec<Protocol>,
    startup_time: Instant,
    last_keep_alive: Instant
}

impl ProtocolThread {
    pub fn start() -> Sender<Protocol> {
        let (tx, rx) = crossbeam_channel::unbounded();

        thread::spawn(move || {
            let now = Instant::now();
            let mut thread = ProtocolThread {
                rx,
                prots: Vec::new(),
                startup_time: now,
                last_keep_alive: now
            };

            loop {
                thread.tick();
                thread::sleep(TICK_DURATION);
            }
        });

        tx
    }

    fn tick(&mut self) {
        self.prots.retain(|x| !x.is_disconnected()); // TODO: destroy clients

        for prot in self.rx.try_iter() {
            self.prots.push(prot);
        }

        let now = Instant::now();
        let keep_alive_id = if now.duration_since(self.last_keep_alive) >= KEEP_ALIVE_INTERVAL {
            self.last_keep_alive = now;
            // vanilla seems to use the raw value of the monotonic clock in milliseconds (at least on Linux)
            // but we can't easily get that in Rust, so we send the monotonic time since startup in milliseconds
            Some(now.duration_since(self.startup_time).as_millis() as i32)
        } else {
            None
        };

        for prot in self.prots.iter_mut() {
            if prot.is_disconnected() {
                // We'll handle it next tick
                continue;
            }

            prot.process_data();
            if let Some(keep_alive_id) = keep_alive_id {
                prot.keep_alive(keep_alive_id);
            }

            prot.handle_out_packets();
        }
    }
}
