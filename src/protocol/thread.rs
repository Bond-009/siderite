use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::{thread, time};

use protocol::Protocol;

pub struct ProtocolThread {

}

impl ProtocolThread {
    pub fn start() -> mpsc::Sender<Protocol> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut protocols = Vec::new();

            loop {
                ProtocolThread::tick(&mut protocols, &rx);
                thread::sleep(time::Duration::from_millis(20));
            }
        });

        tx
    }

    fn tick(prots: &mut Vec<Protocol>, rx: &Receiver<Protocol>) {
        prots.retain(|x| !x.is_disconnected()); // TODO: destroy clients

        for prot in rx.try_iter() {
            prots.push(prot);
        }

        for prot in prots {
            prot.process_data();
            prot.handle_out_packets();
        }
    }
}
