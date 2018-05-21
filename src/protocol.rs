use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::Arc;

use openssl::rsa::Padding;
use nbt::{ReadNBTExt, WriteNBTExt};
use num_traits::FromPrimitive;
use rand::{thread_rng, Rng};

use server::Server;

#[repr(i32)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
enum State {
    HandSchaking = 0x00,
    Status = 0x01,
    Login = 0x02,
    Play = 0x03
}

pub struct Protocol {
    stream: TcpStream,
    server: Arc<Server>,
    state: State,
    encrypted: bool,
    verify_token: [u8; 4]
}

impl Protocol {
    pub fn new(stream: TcpStream, server: Arc<Server>) -> Protocol {
        let mut arr = [0u8; 4];
        thread_rng().fill(&mut arr[..]);
        Protocol {
            stream: stream,
            server: server,
            state: State::HandSchaking,
            encrypted: false,
            verify_token: arr
        }
    }

    pub fn data_received(&mut self) {

        {
            // This packet uses a nonstandard format. It is never length-prefixed
            // and the packet ID is an Unsigned Byte instead of a VarInt.
            // Legacy clients may send this packet to initiate Server List Ping
            let mut tbuf = [0u8];
            self.stream.peek(&mut tbuf).unwrap();
            if tbuf[0] == 0xFE {
                self.handle_legacy_ping();
                self.stream.shutdown(Shutdown::Both).expect("shutdown call failed");
                return;
            }
        }

        loop {
            let lenght = match self.stream.read_var_int() {
                Ok(value) => value,
                //Err(_error) => continue 
                Err(error) =>  {
                    if error.kind() == ErrorKind::UnexpectedEof {
                        info!("Connection terminated");
                        break;
                    }
                    else {
                        continue; // Not enough data
                    }
                }
            };

            let mut rbuf = vec![0u8; lenght as usize];

            match self.stream.read(&mut rbuf).err() {
                Some(error) =>  {
                    if error.kind() == ErrorKind::UnexpectedEof {
                        println!("UnexpectedEof!");
                        break;
                    }
                    else {
                        println!("{}", error);
                        continue;
                    }
                }
                _ => ()
            }

            let mut slice = &rbuf[..];
            let id = slice.read_var_int().unwrap();
            self.handle_packet(&mut slice, id);
        }

        self.stream.shutdown(Shutdown::Both).expect("shutdown call failed");
    }

    fn handle_legacy_ping(&mut self) {
        let _payload = self.stream.read_ubyte().unwrap();
    }

    fn handle_packet(&mut self, rbuf: &[u8], id: i32) {
        debug!("Packet id: {:#X}, state: {:?}", id, self.state);
        match self.state {
            State::HandSchaking => {
                match id {
                    0x00 => self.handle_handschake(rbuf),
                    _ => panic!("Unknown packet: {:#X}, state: {:?}", id, self.state)
                }
            }
            State::Status => {
                match id {
                    0x00 => self.handle_request(),
                    0x01 => self.handle_ping(rbuf),
                    _ => panic!("Unknown packet: {:#X}, state: {:?}", id, self.state)
                }
            }
            State::Login => {
                match id {
                    0x00 => self.login_start(rbuf),
                    0x01 => self.encryption_response(rbuf),
                    _ => panic!("Unknown packet: {:#X}, state: {:?}", id, self.state)
                }
            }
            State::Play => {
                unimplemented!("No game packets implemented")
            }
        }
    }

    // HandSchaking packets:

    fn handle_handschake(&mut self, mut rbuf: &[u8]) {
        let _proto_v = rbuf.read_var_int().unwrap();
        let _server_address = rbuf.read_string().unwrap();
        let _server_port = rbuf.read_ushort().unwrap();
        let next_state = rbuf.read_var_int().unwrap();
        self.state = State::from_i32(next_state).unwrap();
    }

    // Status packets:

    fn handle_request(&mut self) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap();
        // TODO: Add sample
        // TODO: Add favicon
        // TODO
        let response = json!({
            "version": {
                "name": "1.8.9",
                "protocol": 47
            },
            "players": {
                "max": self.server.max_players,
                "online": self.server.online_players(),
                "sample": [
                    {
                        "name": "thinkofdeath",
                        "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
                    }
                ]
            },
            "description": {
                "text": self.server.description,
            }
            //"favicon": "data:image/png;base64,"
        });
        let strres = response.to_string();
        wbuf.write_string(&strres).unwrap();
        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    fn handle_ping(&mut self, mut rbuf: &[u8]) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap();
        let payload = rbuf.read_long().unwrap();
        wbuf.write_long(payload).unwrap();
        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    // Login packets:

    fn login_start(&mut self, mut rbuf: &[u8]) {
        let name = rbuf.read_string().unwrap();
        debug!("Player name: {}", name);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap(); // Encryption Request packet
        wbuf.write_string("").unwrap();
        // Public Key
        wbuf.write_var_int(self.server.public_key_der.len() as i32).unwrap();
        wbuf.write_all(&self.server.public_key_der).unwrap();
        // Verify Token
        wbuf.write_var_int(self.verify_token.len() as i32).unwrap();
        wbuf.write_all(&self.verify_token).unwrap();

        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    fn encryption_response(&mut self, mut rbuf: &[u8]) {
        // Public Key
        let ss_len = rbuf.read_var_int().unwrap();
        let mut ssarr = vec![0u8; ss_len as usize];
        rbuf.read(&mut ssarr).unwrap();
        // Verify Token
        let vt_len = rbuf.read_var_int().unwrap();
        let mut vtarr = vec![0u8; vt_len as usize];
        rbuf.read(&mut vtarr).unwrap();

        let mut vtdvec = vec![0; 128];
        self.server.private_key.private_decrypt(&vtarr, &mut vtdvec, Padding::PKCS1).unwrap();

        if vtdvec != self.verify_token {
            self.disconnect("Hacked client");
            return;
        }

        let mut ssdvec = vec![0; 128];
        self.server.private_key.private_decrypt(&ssarr, &mut ssdvec, Padding::PKCS1).unwrap();
        // TODO: Enable encryption and respond with Login Success packet
    }

    // Other packets:
    fn disconnect(&mut self, reason: &str) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap(); // Disconnect packet
        let reason = json!({
            "text": reason
        });
        wbuf.write_string(&reason.to_string()).unwrap();
        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    // Helper functions:

    fn write_packet(stream: &mut Write, mut rbuf: &[u8]) {
        /*for &byte in rbuf {
            print!("{:X} ", byte);
        }*/
        let lenght = rbuf.len() as i32;
        debug!("wbuf len {}, id: {:#X}", lenght, rbuf.first().unwrap());
        stream.write_var_int(lenght).unwrap(); // Write packet lenght
        stream.write_all(&mut rbuf).unwrap();
    }
}
