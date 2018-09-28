pub mod authenticator;

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, RwLock};

use circbuf::CircBuf;
use openssl::rsa::Padding;
use openssl::symm::{encrypt, Cipher};
use nbt::{ReadNBTExt, WriteNBTExt};
use num_traits::FromPrimitive;
use rand::{thread_rng, Rng};

use client::Client;
use world::World;

#[repr(i32)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
enum State {
    HandSchaking = 0x00,
    Status = 0x01,
    Login = 0x02,
    Play = 0x03
}

pub struct Protocol {
    client: Arc<RwLock<Client>>,
    stream: TcpStream,
    state: State,
    received_data: CircBuf,
    encrypted: bool,
    verify_token: [u8; 4],
    cipher: Cipher,
    encryption_key: Vec<u8>, //[u8; 16]
}

impl Protocol {
    pub fn new(client: Arc<RwLock<Client>>, stream: TcpStream) -> Protocol {
        let mut arr = [0u8; 4];
        thread_rng().fill(&mut arr[..]);
        Protocol {
            client: client,
            stream: stream,
            state: State::HandSchaking,
            received_data: CircBuf::with_capacity(32 * 1024).unwrap(),
            encrypted: false,
            verify_token: arr,
            cipher: Cipher::aes_128_cfb128(),
            encryption_key: Vec::new(), // [0u8; 16]
        }
    }

    pub fn legacy_ping(mut stream: &mut TcpStream) -> bool {
        // This packet uses a nonstandard format. It is never length-prefixed
        // and the packet ID is an Unsigned Byte instead of a VarInt.
        // Legacy clients may send this packet to initiate Server List Ping
        let mut tbuf = [0u8];
        stream.peek(&mut tbuf).unwrap();
        if tbuf[0] == 0xFE {
            Protocol::handle_legacy_ping(&mut stream);
            stream.shutdown(Shutdown::Both).expect("shutdown call failed");
            return true;
        }
        return false;
    }

    fn handle_legacy_ping(_stream: &mut TcpStream) {
        //let payload = stream.read_ubyte().unwrap();
        //assert_eq!(payload, 1);
    }

    // In

    pub fn process_data(&mut self) {
        let mut tmp = [0u8; 512];
        let len = self.stream.peek(&mut tmp).unwrap(); // or_error()
        drop(tmp);

        if len == 0 {
            // Nothing to read, return
            return;
        }

        if self.encrypted {
            unimplemented!();
            /*
            let mut vec = vec![0u8; len];
            self.stream.read(&mut vec).unwrap();
            let mut dvec = decrypt(self.cipher, &self.encryption_key, Some(&self.encryption_key), &vec).unwrap();
            self.received_data.write(&dvec).unwrap();
            */
        }
        else {
            let mut vec = vec![0u8; len];
            self.stream.read(&mut vec).unwrap();

            // debug!("{}", util::array_as_hex(&vec)); // Full rec data dump

            self.received_data.write(&vec).unwrap();
        }
    }

    pub fn handle_in_packets(&mut self) {
        loop {
            if self.received_data.len() == 0 {
                // No data
                return;
            }

            let lenght = match self.received_data.read_var_int() {
                Ok(value) => value as usize,
                Err(error) => {
                    if error.kind() == ErrorKind::UnexpectedEof {
                        info!("Connection terminated");
                        return;
                    }
                    else {
                        debug!("Not enough data");
                        return; // Not enough data
                    }
                }
            };

            debug!("Packet lenght: {}", lenght);

            if self.received_data.len() < lenght {
                return; // Not enough data
            }

            let mut rbuf = vec![0u8; lenght];
            match self.received_data.read(&mut rbuf).err() {
                Some(error) => {
                    if error.kind() == ErrorKind::UnexpectedEof {
                        println!("UnexpectedEof!");
                        return;
                    }
                    else {
                        println!("Err: {}", error);
                        return;
                    }
                }
                _ => ()
            }

            let mut slice = &rbuf[..];
            let id = slice.read_var_int().unwrap();
            self.handle_packet(&mut slice, id);
        }
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
                    0x00 => self.handle_login_start(rbuf),
                    0x01 => self.handle_encryption_response(rbuf),
                    _ => panic!("Unknown packet: {:#X}, state: {:?}", id, self.state)
                }
            }
            State::Play => {
                unimplemented!("No packets implemented")
            }
        }
    }

    // Out:

    fn _send_packet(&mut self, packet: Packet) {
        match packet {
            Packet::Disconnect(reason) => self.disconnect(&reason)
        }
    }

    fn write_packet(&mut self, mut rbuf: &[u8]) {
        let lenght = rbuf.len() as i32;
        debug!("Write packet: len {}, id: {:#X}", lenght, rbuf.first().unwrap());
        let mut vec = Vec::with_capacity(rbuf.len() + 4);
        vec.write_var_int(lenght).unwrap(); // Write packet lenght
        vec.write_all(&mut rbuf).unwrap(); // Write packet data

        if self.encrypted {
            vec = encrypt(self.cipher, &self.encryption_key, Some(&self.encryption_key), &vec).unwrap();
        }
        self.stream.write(&mut vec).unwrap();
    }

    // HandSchaking packets:

    fn handle_handschake(&mut self, mut rbuf: &[u8]) {
        let proto_v = rbuf.read_var_int().unwrap();
        debug_assert_eq!(proto_v, 47);
        let _server_address = rbuf.read_string().unwrap();
        let _server_port = rbuf.read_ushort().unwrap();
        let next_state = rbuf.read_var_int().unwrap();
        self.state = State::from_i32(next_state).unwrap();
        debug!("Changed State to {:?}", self.state);
    }

    // Status packets:

    fn handle_request(&mut self) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap();
        // TODO: Add sample
        // TODO: Add favicon
        // TODO
        let server = self.client.read().unwrap().get_server();
        let response = json!({
            "version": {
                "name": "1.8.9",
                "protocol": 47
            },
            "players": {
                "max": server.max_players,
                "online": server.online_players(),
                "sample": [
                    {
                        "name": "thinkofdeath",
                        "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
                    }
                ]
            },
            "description": {
                "text": server.description,
            }
            //"favicon": "data:image/png;base64,"
        });
        let strres = response.to_string();
        wbuf.write_string(&strres).unwrap();
        self.write_packet(&wbuf);
    }

    fn handle_ping(&mut self, mut rbuf: &[u8]) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap();
        let payload = rbuf.read_long().unwrap();
        debug!("Ping payload: {}", payload);
        wbuf.write_long(payload).unwrap();
        self.write_packet(&wbuf);
    }

    // Login packets:

    fn handle_login_start(&mut self, mut rbuf: &[u8]) {
        let username = rbuf.read_string().unwrap();
        debug!("Player name: {}", username);

        let server = self.client.read().unwrap().get_server();
        if server.authenticate {
            let mut wbuf = Vec::new();
            wbuf.write_var_int(0x01).unwrap(); // Encryption Request packet
            wbuf.write_string(&server.id).unwrap();
            // Public Key
            wbuf.write_var_int(server.public_key_der.len() as i32).unwrap();
            wbuf.write_all(&server.public_key_der).unwrap();
            // Verify Token
            wbuf.write_var_int(self.verify_token.len() as i32).unwrap();
            wbuf.write_all(&self.verify_token).unwrap();

            self.write_packet(&wbuf);
        }

        self.client.write().unwrap().handle_login(username);
        self.disconnect("Not implemented yet");
    }

    fn handle_encryption_response(&mut self, mut rbuf: &[u8]) {
        // Public Key
        let ss_len = rbuf.read_var_int().unwrap();
        let mut ssarr = vec![0u8; ss_len as usize];
        rbuf.read(&mut ssarr).unwrap();
        // Verify Token
        let vt_len = rbuf.read_var_int().unwrap();
        let mut vtarr = vec![0u8; vt_len as usize];
        rbuf.read(&mut vtarr).unwrap();

        let server = self.client.read().unwrap().get_server();

        let mut vtdvec = vec![0; 128];
        server.private_key.private_decrypt(&vtarr, &mut vtdvec, Padding::PKCS1).unwrap();

        if vtdvec != self.verify_token {
            self.disconnect("Hacked client");
            return;
        }

        let mut ssdvec = vec![0; 128];
        server.private_key.private_decrypt(&ssarr, &mut ssdvec, Padding::PKCS1).unwrap();

        self.encrypted = true;
        self.encryption_key = ssarr;

        self.login_success();
    }

    fn login_success(&mut self) {
        assert_eq!(self.state, State::Login);

        self.state = State::Play;

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x02).unwrap(); // Login packet

        {
            let client = self.client.read().unwrap();

            wbuf.write_string("4566e69f-c907-48ee-8d71-d7ba5aa00d20").unwrap(); // TODO: UUID
            wbuf.write_string(&client.get_username().unwrap()).unwrap(); // TODO
        }

        self.write_packet(&wbuf);
    }

    // Play packets:

    fn _join_game(&mut self, _world: &World) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x02).unwrap(); // Join Game packet

        wbuf.write_int(0).unwrap();
        wbuf.write_ubyte(1).unwrap();
        wbuf.write_byte(0).unwrap(); // TODO: add
        wbuf.write_ubyte(0).unwrap(); // TODO: add

        let max_players = self.client.read().unwrap().get_server().max_players;

        wbuf.write_ubyte(max_players as u8).unwrap();
        wbuf.write_string(&"default").unwrap(); // Level Type?
        wbuf.write_bool(false).unwrap(); // Reduced debug info?

        self.write_packet(&wbuf);
    }

    // Other packets:
    fn disconnect(&mut self, reason: &str) {
        let mut wbuf = Vec::new();
        wbuf.write_var_int(
            match self.state {
                State::Login => 0x00,
                State::Play => 0x40,
                _ => panic!("Unknown state for Disconnect Packet: {:?}", self.state)
            }
        ).unwrap(); // Disconnect packet
        let reason = json!({
            "text": reason
        });
        wbuf.write_string(&reason.to_string()).unwrap();
        self.write_packet(&wbuf);
    }
}
