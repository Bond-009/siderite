pub mod packets;
pub mod thread;
mod v47;

use std::io::{ErrorKind, Read, Result, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use bytebufrs::RingBuf;
use crossbeam_channel::Receiver;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use log::*;
use mcrw::{MCReadExt, MCWriteExt};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use openssl::rsa::Padding;
use openssl::sha::Sha1;
use openssl::symm::{Cipher, Crypter, Mode};
use rand::{Rng, rng};
use serde_json::json;

use crate::auth;
use crate::blocks::BlockFace;
use crate::client::Client;
use crate::coord::{ChunkCoord, Coord};
use crate::entities::player::{Abilities, Player, SkinFlags};
use crate::server;
use crate::server::Server;
use crate::storage::chunk::chunk_map::ChunkMap;
use crate::storage::chunk::{Chunk, SerializeChunk};
use crate::storage::world::{Difficulty, World};

use self::packets::{Packet, PlayerListAction};

/// The length of the verify token
const VERIFY_TOKEN_LEN: usize = 4;

/// The length of the encryption key
const ENCRYPTION_KEY_LEN: usize = 16;

const PADDING: Padding = Padding::PKCS1;

/// Maximum duration in between keep alive packets from the client
const KEEP_ALIVE_MAX: Duration = Duration::from_secs(30);

#[repr(i32)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
enum State {
    HandShaking = 0x00,
    Status = 0x01,
    Login = 0x02,
    Play = 0x03,
    Disconnected = 0xFF,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive)]
pub enum GameStateReason {
    /// Bed can't be used as a spawn point
    InvalidBed = 0,
    EndRaining = 1,
    BeginRaining = 2,
    /// 0: Survival, 1: Creative, 2: Adventure, 3: Spectator
    ChangeGameMode = 3,
    EnterCredits = 4,
    /// 0: Show welcome to demo screen,
    /// 101: Tell movement controls,
    /// 102: Tell jump control,
    /// 103: Tell inventory control
    DemoMessage = 5,
    /// Appears to be played when an arrow strikes another player in Multiplayer
    ArrowHittingPlayer = 6,
    /// The current darkness value. 1 = Dark, 0 = Bright,
    /// Setting the value higher causes the game to change color and freeze
    FadeValue = 7,
    /// Time in ticks for the sky to fade
    FadeTime = 8,
    /// Unknown
    PlayMobAppearance = 10,
}

#[repr(i8)]
#[derive(Copy, Clone, Debug, FromPrimitive)]
pub enum DigStatus {
    StartedDigging = 0,
    CancelledDigging = 1,
    FinishedDigging = 2,
    DropItemStack = 3,
    DropItem = 4,
    ShootArrowFinishEating = 5,
}

pub struct Protocol {
    server: Arc<Server>,
    client_id: u32,
    client: Arc<RwLock<Client>>,
    receiver: Receiver<Packet>,

    stream: TcpStream,
    state: State,
    received_data: RingBuf,
    compressed: bool,

    last_keep_alive: Instant,

    verify_token: [u8; VERIFY_TOKEN_LEN],
    encryption_key: [u8; ENCRYPTION_KEY_LEN],
    crypter: Option<(Crypter, Crypter)>,
}

impl Protocol {
    pub fn new(server: Arc<Server>, stream: TcpStream) -> Self {
        let mut arr = [0u8; VERIFY_TOKEN_LEN];
        rng().fill(arr.as_mut_slice());
        let (tx, rx) = crossbeam_channel::unbounded();
        // The player will get the same ID as the client
        let client_id = server::get_next_entity_id();
        Self {
            server: server.clone(),
            client_id,
            client: Arc::new(RwLock::new(Client::new(client_id, server, tx))),
            receiver: rx,

            stream,
            state: State::HandShaking,
            received_data: RingBuf::with_capacity((32 * 1024) - 1),
            compressed: false,

            last_keep_alive: Instant::now(),

            verify_token: arr,
            encryption_key: [0u8; ENCRYPTION_KEY_LEN],
            crypter: None,
        }
    }

    pub fn get_client(&self) -> (u32, Arc<RwLock<Client>>) {
        (self.client_id, self.client.clone())
    }

    pub fn is_disconnected(&self) -> bool {
        self.state == State::Disconnected
    }

    /// Checks if the first packet is a legacy ping packet (MC v1.4 - 1.6)
    /// If it is, handles it and returns true
    pub fn legacy_ping(mut stream: &mut TcpStream) -> bool {
        // This packet uses a nonstandard format. It is never length-prefixed
        // and the packet ID is an Unsigned Byte instead of a VarInt.
        // Legacy clients may send this packet to initiate Server List Ping
        let mut tbuf = [0u8];
        let len = stream.peek(&mut tbuf).unwrap();
        if len == 1 && tbuf[0] == 0xFE {
            stream.read_exact(&mut tbuf).unwrap();
            Protocol::handle_legacy_ping(&mut stream);
            stream
                .shutdown(Shutdown::Both)
                .expect("shutdown call failed");
            return true;
        }

        false
    }

    fn handle_legacy_ping(stream: &mut TcpStream) {
        // server list ping's payload (always 1)
        let payload = stream.read_ubyte().unwrap();
        assert_eq!(payload, 1);

        // packet identifier for a plugin message
        let _packet_id = stream.read_ubyte().unwrap();

        // length of following string, in characters, as a short (always 11)
        // "MC|PingHost" encoded as a UTF-16BE string
        let len = stream.read_ushort().unwrap();
        assert_eq!(len, 11);
        let mut string = vec![0u8; (len * 2) as usize];
        stream.read_exact(&mut string).unwrap();

        // length of the rest of the data, as a short
        let _rest_len = stream.read_ushort().unwrap();

        let _prot_ver = stream.read_ubyte().unwrap();
        let len = stream.read_ushort().unwrap();
        let mut string = vec![0u8; (len * 2) as usize];
        stream.read_exact(&mut string).unwrap();

        let _port = stream.read_int().unwrap();

        // TODO: respond
    }

    // In

    pub fn process_data(&mut self) {
        let mut tmp = [0u8; 512];
        let len = match self.stream.peek(&mut tmp) {
            Ok(v) => v,
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // return, we don't want to block the protocols thread
                return;
            }
            Err(ref e) if Protocol::is_disconnection_error(e.kind()) => {
                // Connection closed
                self.state = State::Disconnected;
                return;
            }
            Err(e) => {
                warn!("Encountered IO error: {}", e);
                self.shutdown().unwrap();
                return;
            }
        };

        if len == 0 {
            // Connection closed
            if let Err(e) = self.shutdown() {
                if !Protocol::is_disconnection_error(e.kind()) {
                    warn!("Error while shutting down connection: {}", e);
                }
            }

            return;
        }

        let mut vec = vec![0u8; len];
        self.stream.read_exact(&mut vec).unwrap();

        match &mut self.crypter {
            Some((_, de)) => {
                let mut dvec = vec![0u8; len];
                let dlen = de.update(&vec, &mut dvec).unwrap();
                self.received_data.write_all(&dvec[..dlen]).unwrap();
            }
            None => self.received_data.write_all(&vec).unwrap(),
        }

        self.handle_in_packets();
    }

    fn handle_in_packets(&mut self) {
        let mut buf = [0u8; mcrw::VAR_INT_MAX_SIZE];
        while self.state != State::Disconnected {
            let length = match self.received_data.peek(&mut buf) {
                Ok(0) | Err(_) => {
                    return; // Not enough data
                }
                Ok(read) => {
                    match (&buf[..read]).read_var_int() {
                        Ok(v) => v as usize,
                        Err(_) => {
                            return; // Not enough data
                        }
                    }
                }
            };

            if self.received_data.len() < length {
                return; // Not enough data
            }

            self.received_data
                .advance_read_pos(mcrw::var_int_size(length as i32))
                .unwrap();

            debug!("Packet length: {}", length);

            let mut rbuf = vec![0u8; length];
            let read = self.received_data.read(&mut rbuf).unwrap();
            debug_assert_eq!(read, length);
            let mut rslice = rbuf.as_slice();

            if self.compressed {
                let data_length = rslice.read_var_int().unwrap();
                debug!("Data length: {}", length);
                if data_length != 0 {
                    let mut d = ZlibDecoder::new(rslice);
                    let mut vec = vec![0u8; data_length as usize];
                    d.read_exact(&mut vec).unwrap();
                    let mut slice = vec.as_slice();
                    let id = slice.read_var_int().unwrap();
                    self.handle_packet(slice, id);
                    return;
                }
            }

            let id = rslice.read_var_int().unwrap();
            self.handle_packet(rslice, id);
        }
    }

    fn handle_packet(&mut self, rbuf: &[u8], id: i32) {
        match self.state {
            State::HandShaking => match id {
                0x00 => self.handle_handshake(rbuf),
                _ => {
                    self.unknown_packet(id);
                    self.shutdown().unwrap();
                }
            },
            State::Status => {
                let res = match id {
                    0x00 => self.handle_request(),
                    0x01 => self.handle_ping(rbuf),
                    _ => {
                        self.unknown_packet(id);
                        self.shutdown()
                    }
                };

                if let Err(e) = res {
                    error!("Error while handling packets: {}", e);
                    self.state = State::Disconnected;
                }
            }
            State::Login => {
                let res = match id {
                    0x00 => self.handle_login_start(rbuf),
                    0x01 => self.handle_encryption_response(rbuf),
                    _ => {
                        self.unknown_packet(id);
                        self.disconnect(&format!("Unknown packet: {:#X}", id))
                    }
                };

                if let Err(e) = res {
                    error!("Error while handling packets: {}", e);
                    self.state = State::Disconnected;
                }
            }
            State::Play => {
                match id {
                    0x00 => self.handle_keep_alive(rbuf),
                    0x01 => self.handle_chat_message(rbuf),
                    0x03 => self.handle_player(rbuf),
                    0x04 => self.handle_player_pos(rbuf),
                    0x05 => self.handle_player_look(rbuf),
                    0x06 => self.handle_player_pos_look(rbuf),
                    0x07 => self.handle_player_digging(rbuf),
                    0x08 => self.handle_player_block_placement(rbuf),
                    0x09 => self.handle_held_item_change(rbuf),
                    0x0A => (), // Sent when the player's arm swings
                    0x0B => self.handle_entity_action(rbuf),
                    0x0D => self.handle_close_window(rbuf),
                    0x0E => self.handle_click_window(rbuf),
                    0x10 => self.handle_creative_inventory_action(rbuf),
                    0x13 => self.handle_player_abilities(rbuf),
                    0x15 => self.handle_client_settings(rbuf),
                    0x16 => self.handle_client_status(rbuf),
                    0x17 => self.handle_plugin_message(rbuf),
                    _ => {
                        self.unknown_packet(id);
                        self.disconnect(&format!("Unknown packet: {:#X}", id))
                            .unwrap();
                    }
                }
            }
            State::Disconnected => {} // Ignore all packets
        }
    }

    fn unknown_packet(&self, id: i32) {
        error!("Unknown packet: {:#X}, state: {:?}", id, self.state);
    }

    // Out:

    pub fn handle_out_packets(&mut self) {
        if self.state == State::Disconnected {
            // Don't send packets when in disconnected state
            return;
        }

        let mut packets = Vec::new();
        for p in self.receiver.try_iter() {
            packets.push(p);
        }

        for p in packets {
            self.send_packet(p);
        }
    }

    fn send_packet(&mut self, packet: Packet) {
        let res = match packet {
            Packet::LoginSuccess() => self.login_success(),

            Packet::ChatMessage(raw_message) => self.chat_message(raw_message),
            Packet::JoinGame(player, world) => self.join_game(player, world),
            Packet::TimeUpdate(world) => self.time_update(world),
            Packet::SpawnPosition(world) => self.spawn_position(world),
            Packet::PlayerPositionAndLook(player) => self.player_pos_look(player),
            Packet::SpawnPlayer(player) => self.spawn_player(player),
            Packet::ChangeGameState(reason, value) => self.change_game_state(reason, value),
            Packet::PlayerListItem(action, players) => self.player_list_item(action, players),
            Packet::PlayerAbilities(player) => self.player_abilities(player),
            Packet::ChunkData(coord, chunk_map) => self.chunk_data(coord, chunk_map),
            Packet::ServerDifficulty(difficulty) => self.server_difficulty(difficulty),
            Packet::ResourcePackSend(url, hash) => self.resource_pack_send(&url, &hash),

            Packet::Disconnect(reason) => self.disconnect(&reason),
        };

        if res.is_err() {
            // We don't care about the result
            self.shutdown().unwrap();
        }
    }

    fn write_packet(&mut self, rbuf: &[u8]) -> Result<()> {
        let length = rbuf.len() as i32;
        debug!(
            "Write packet: state: {:?}, len {}, id: {:#X}",
            self.state, length, rbuf[0]
        );

        // REVIEW: duplicate code + multiple writes to self.stream
        match &mut self.crypter {
            Some((en, _)) => {
                let mut buf = vec![0; rbuf.len() + 10];
                if !self.compressed {
                    buf.write_var_int(length)?; // Write packet length
                    buf.write_all(&rbuf)?; // Write packet data
                } else if length < self.server.compression_threshold().unwrap() {
                    buf.write_var_int(length + 1)?; // Write packet length
                    buf.write_var_int(0)?;
                    buf.write_all(&rbuf)?;
                } else {
                    let mut zen =
                        ZlibEncoder::new(Vec::with_capacity(rbuf.len()), Compression::default());
                    zen.write_all(rbuf)?;
                    let comp_buf = zen.finish()?;
                    buf.write_var_int((mcrw::var_int_size(length) + comp_buf.len()) as i32)?;
                    buf.write_var_int(length)?;
                    buf.write_all(&comp_buf)?;
                }

                let mut enc_buf = vec![0; buf.len() + 128];
                let enc_len = en.update(&buf, &mut enc_buf).unwrap();
                self.stream.write_all(&enc_buf[..enc_len])?;
            }
            None => {
                if !self.compressed {
                    self.stream.write_var_int(length)?; // Write packet length
                    self.stream.write_all(&rbuf)?; // Write packet data
                } else if length < self.server.compression_threshold().unwrap() {
                    self.stream.write_var_int(length + 1)?; // Write packet length
                    self.stream.write_var_int(0)?;
                    self.stream.write_all(&rbuf)?;
                } else {
                    let mut zen =
                        ZlibEncoder::new(Vec::with_capacity(rbuf.len()), Compression::default());
                    zen.write_all(rbuf)?;
                    let comp_buf = zen.finish()?;
                    self.stream
                        .write_var_int((mcrw::var_int_size(length) + comp_buf.len()) as i32)?;
                    self.stream.write_var_int(length)?;
                    self.stream.write_all(&comp_buf)?;
                }
            }
        }

        Ok(())
    }

    // HandShaking packets:

    fn handle_handshake(&mut self, mut rbuf: &[u8]) {
        let _proto_v = rbuf.read_var_int().unwrap();
        let _server_address = rbuf.read_string().unwrap();
        let _server_port = rbuf.read_ushort().unwrap();
        let next_state = rbuf.read_var_int().unwrap();
        self.state = State::from_i32(next_state).unwrap();
        debug!("Changed State to {:?}", self.state);
    }

    // Status packets:

    fn handle_request(&mut self) -> Result<()> {
        debug_assert_eq!(self.state, State::Status);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap();
        let mut response = json!({
            "version": {
                "name": "1.8.9",
                "protocol": 47
            },
            "players": {
                "max": self.server.max_players(),
                "online": self.server.online_players(),
                "sample": [
                    {
                        "name": "thinkofdeath",
                        "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
                    }
                ]
            },
            "description": {
                "text": self.server.motd(),
            },
        });
        if let Some(favicon) = self.server.favicon() {
            response.as_object_mut().unwrap().insert(
                "favicon".to_owned(),
                json!(format!("data:image/png;base64,{}", favicon)),
            );
        }

        let strres = response.to_string();
        debug!("{}", strres);
        wbuf.write_string(&strres).unwrap();
        self.write_packet(&wbuf)
    }

    fn handle_ping(&mut self, mut rbuf: &[u8]) -> Result<()> {
        debug_assert_eq!(self.state, State::Status);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap();
        let payload = rbuf.read_long().unwrap();
        debug!("Ping payload: {}", payload);
        wbuf.write_long(payload).unwrap();
        self.write_packet(&wbuf)
    }

    // Login packets:

    fn handle_login_start(&mut self, mut rbuf: &[u8]) -> Result<()> {
        let username = rbuf.read_string().unwrap();
        self.client.write().unwrap().set_username(username);

        if self.server.encryption() {
            return self.encryption_request();
        } else {
            self.client.write().unwrap().handle_login(None);
        }

        Ok(())
    }

    fn handle_encryption_response(&mut self, mut rbuf: &[u8]) -> Result<()> {
        let ss_len = rbuf.read_var_int().unwrap() as usize; // Shared Secret Key Length
        let mut ssarr = vec![0u8; ss_len];
        rbuf.read_exact(&mut ssarr).unwrap(); // Shared Secret

        let vt_len = rbuf.read_var_int().unwrap() as usize; // Verify Token Length
        let mut vtarr = vec![0u8; vt_len];
        rbuf.read_exact(&mut vtarr).unwrap(); // Verify Token

        let private_key = self.server.private_key();

        // Decrypt the and verify the Verify Token
        let mut vtdvec = vec![0; vt_len];
        let vtd_len = private_key
            .private_decrypt(&vtarr, &mut vtdvec, PADDING)
            .unwrap();
        if vtd_len != VERIFY_TOKEN_LEN {
            debug!(
                "Verify Token is the wrong length: expected {}, got {}",
                VERIFY_TOKEN_LEN, vtd_len
            );
            self.disconnect("Hacked client")?;
            return Ok(());
        }

        if vtdvec[..VERIFY_TOKEN_LEN] != self.verify_token[..] {
            debug!("Verify Token is not the same");
            self.disconnect("Hacked client")?;
            return Ok(());
        }

        // Decrypt Shared Secret Key
        let mut ssdvec = vec![0; ss_len];
        let ssd_len = private_key
            .private_decrypt(&ssarr, &mut ssdvec, PADDING)
            .unwrap();
        if ssd_len != ENCRYPTION_KEY_LEN {
            debug!(
                "Shared Secret Key is the wrong length: expected {}, got {}",
                ENCRYPTION_KEY_LEN, ssd_len
            );
            self.disconnect("Hacked client")?;
            return Ok(());
        }

        self.encryption_key
            .copy_from_slice(&ssdvec[..ENCRYPTION_KEY_LEN]);

        // AES/CFB8 cipher used by minecraft
        let cipher = Cipher::aes_128_cfb8();
        let encrypter = Crypter::new(
            cipher,
            Mode::Encrypt,
            &self.encryption_key,
            Some(&self.encryption_key),
        )
        .unwrap();
        let decrypter = Crypter::new(
            cipher,
            Mode::Decrypt,
            &self.encryption_key,
            Some(&self.encryption_key),
        )
        .unwrap();
        self.crypter = Some((encrypter, decrypter));

        let mut hasher = Sha1::new();
        hasher.update(self.server.id().as_bytes());
        hasher.update(&self.encryption_key);
        hasher.update(&self.server.public_key_der());
        let hash = hasher.finish();
        let server_id = auth::java_hex_digest(hash);
        self.client.read().unwrap().handle_login(Some(server_id));

        Ok(())
    }

    fn encryption_request(&mut self) -> Result<()> {
        debug_assert_eq!(self.state, State::Login);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap(); // Encryption Request packet
        wbuf.write_string(&self.server.id()).unwrap();
        // Public Key
        let public_key_der = self.server.public_key_der();
        wbuf.write_var_int(public_key_der.len() as i32).unwrap();
        wbuf.write_all(public_key_der).unwrap();
        // Verify Token
        wbuf.write_var_int(self.verify_token.len() as i32).unwrap();
        wbuf.write_all(&self.verify_token).unwrap();

        self.write_packet(&wbuf)
    }

    fn login_success(&mut self) -> Result<()> {
        debug_assert_eq!(self.state, State::Login);

        // Enable compression
        if let Some(compression_threshold) = self.server.compression_threshold() {
            self.set_compression(compression_threshold)?;
        }

        self.state = State::Play;
        debug!("Changed State to {:?}", self.state);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x02).unwrap(); // Login Success packet

        {
            let client = self.client.read().unwrap();

            let uuid = client.uuid().as_hyphenated().to_string();
            let username = client.get_username().unwrap();
            debug!("uuid: {}", uuid);
            debug!("name: {}", username);

            wbuf.write_string(&uuid).unwrap();
            wbuf.write_string(&username).unwrap();
        }

        self.write_packet(&wbuf)
    }

    fn set_compression(&mut self, threshold: i32) -> Result<()> {
        debug_assert_eq!(self.state, State::Login);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x03).unwrap(); // Login Success packet

        // Maximum size of a packet before its compressed
        wbuf.write_var_int(threshold).unwrap(); // Threshold

        self.write_packet(&wbuf)?;
        self.compressed = true;

        Ok(())
    }

    // Play packets:

    /// The server will frequently send out a keep-alive, each containing a random ID.
    /// The client must respond with the same packet.
    fn handle_keep_alive(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _id = rbuf.read_var_int().unwrap();
        let now = Instant::now();
        if now.duration_since(self.last_keep_alive) >= KEEP_ALIVE_MAX {
            self.disconnect("Timed out!").unwrap();
            return;
        }

        self.last_keep_alive = now;
    }

    /// Check the message to see if it begins with a '/'.
    /// If it does, the server assumes it to be a command and attempts to process it.
    /// If it doesn't, the username of the sender is prepended and sent to all clients.
    fn handle_chat_message(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let msg = rbuf.read_string().unwrap();
        if msg.starts_with('/') {
            // TODO: exec cmd
            return;
        }

        let client = self.client.read().unwrap();
        let username = client.get_username().unwrap();
        self.server.broadcast_chat(username, &msg);
    }

    /// This packet is used to indicate whether the player is on ground (walking/swimming),
    /// or airborne (jumping/falling).
    fn handle_player(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _on_ground = rbuf.read_bool().unwrap();
    }

    /// Updates the player's XYZ position on the server.
    fn handle_player_pos(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // Feet pos
        let _x = rbuf.read_double().unwrap();
        let _y = rbuf.read_double().unwrap();
        let _z = rbuf.read_double().unwrap();
        let _on_ground = rbuf.read_bool().unwrap();
    }

    /// Updates the direction the player is looking in.
    fn handle_player_look(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _yaw = rbuf.read_float().unwrap();
        let _pitch = rbuf.read_float().unwrap();
        let _on_ground = rbuf.read_bool().unwrap();
    }

    /// A combination of Player Look and Player Position.
    fn handle_player_pos_look(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // TODO: Do something
        // Feet pos
        let _x = rbuf.read_double().unwrap();
        let _y = rbuf.read_double().unwrap();
        let _z = rbuf.read_double().unwrap();

        let _yaw = rbuf.read_float().unwrap();
        let _pitch = rbuf.read_float().unwrap();
        let _on_ground = rbuf.read_bool().unwrap();
    }

    /// Sent when the player mines a block. A Notchian server only accepts
    /// digging packets with coordinates within a 6-unit radius of the player's position.
    fn handle_player_digging(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let status = rbuf.read_byte().unwrap();
        let (x, y, z) = rbuf.read_position().unwrap();

        let face = rbuf.read_byte().unwrap();
        debug_assert!(face >= 0 && face < 6);

        let client = self.client.read().unwrap();
        client.handle_left_click(
            Coord {
                x: x as i32,
                y: y as i32,
                z: z as i32,
            },
            BlockFace::from_i8(face).unwrap(),
            DigStatus::from_i8(status).unwrap(),
        );
    }

    /// Sent when the player changes the slot selection
    fn handle_player_block_placement(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let (_x, _y, _z) = rbuf.read_position().unwrap();
        // See packet above for explanation
        let _face = rbuf.read_byte().unwrap();
        // TODO read slot

        // let _cursor_x = rbuf.read_byte().unwrap();
        // let _cursor_y = rbuf.read_byte().unwrap();
        // let _cursor_z = rbuf.read_byte().unwrap();
    }

    /// Sent when the player changes the slot selection
    fn handle_held_item_change(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let slot = rbuf.read_short().unwrap();
        debug_assert!(slot >= 0 && slot < 9, "Invalid slot number");
    }

    /// Sent by the client to indicate that it has performed certain actions:
    /// sneaking (crouching), sprinting, exiting a bed, jumping with a horse,
    /// and opening a horse's inventory while riding it.
    fn handle_entity_action(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // TODO: Do something

        let _entity_id = rbuf.read_var_int().unwrap(); // Entity ID
        let _action_id = rbuf.read_var_int().unwrap(); // Action ID
        // Only used by Horse Jump Boost, in which case it ranges from 0 to 100. In all other cases it is 0.
        let _action_par = rbuf.read_var_int().unwrap(); // Action Parameter

        // ID | Action
        // --------------------------------
        // 0  | Start sneaking
        // 1  | Stop sneaking
        // 2  | Leave bed
        // 3  | Start sprinting
        // 4  | Stop sprinting
        // 5  | Jump with horse
        // 6  | Open ridden horse inventory
    }

    /// This packet is sent by the client when closing a window.
    /// Notchian clients send a Close Window packet with Window ID 0 to close their inventory
    /// even though there is never an Open Window packet for the inventory.
    fn handle_close_window(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _window_id = rbuf.read_ubyte().unwrap(); // Window ID
    }

    /// This packet is sent by the player when it clicks on a slot in a window.
    fn handle_click_window(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _window_id = rbuf.read_ubyte().unwrap(); // Window ID
        let _slot = rbuf.read_short().unwrap(); // Slot
        let _button = rbuf.read_byte().unwrap(); // Button
        let _action = rbuf.read_short().unwrap(); // Action Number
        let _mode = rbuf.read_ubyte().unwrap(); // Inventory operation mode
        // TODO: Read slot
    }

    /// While the user is in the standard inventory (i.e., not a crafting bench) in Creative mode,
    /// the player will send this packet.
    fn handle_creative_inventory_action(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _slot = rbuf.read_short().unwrap();
        // TODO: handle slot data
    }

    /// The latter 2 values are used to indicate the walking and flying speeds respectively,
    /// while the first byte is used to determine the value of 4 booleans.
    /// The vanilla client sends this packet when the player starts/stops flying
    /// with the Flags parameter changed accordingly. All other parameters are ignored by the vanilla server.
    fn handle_player_abilities(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _abilities = Abilities::from_bits_truncate(rbuf.read_ubyte().unwrap());
        let _flying_speed = rbuf.read_float().unwrap();
        let _walking_speed = rbuf.read_float().unwrap();
    }

    /// Sent when the player connects, or when settings are changed.
    fn handle_client_settings(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // TODO: Do something with the settings
        let locale = rbuf.read_string().unwrap();
        debug!("Locale: {}", locale);
        let view_distance = rbuf.read_byte().unwrap();
        debug!("View Distance: {}", view_distance);
        // TODO: create an enum
        let _bchat_mode = rbuf.read_byte().unwrap();
        let _chat_colors = rbuf.read_bool().unwrap();
        // Bit      | Meaning
        // ----------------------------------
        // 0 (0x01) | Cape enabled
        // 1 (0x02) | Jacket enabled
        // 2 (0x04) | Left Sleeve enabled
        // 3 (0x08) | Right Sleeve enabled
        // 4 (0x10) | Left Pants Leg enabled
        // 5 (0x20) | Right Pants Leg enabled
        // 6 (0x40) | Hat enabled
        // 7 (0x80) | !Unused
        let _skin_parts = SkinFlags::from_bits_truncate(rbuf.read_ubyte().unwrap());
    }

    /// Sent when the client is ready to complete login and when the client is ready to respawn after death.
    fn handle_client_status(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let action_id = rbuf.read_var_int().unwrap(); // Action ID

        // Action ID | Action
        // ----------------------------------------
        // 0         | Perform respawn
        // 1         | Request stats
        // 2         | Taking Inventory achievement

        match action_id {
            0 => (), // TODO: respawn
            1 => (), // TODO: Stats
            2 => (), // TODO // Taking Inventory achievement
            _ => {
                error!("Action ID is out of range (0..2), got {}", action_id);
                self.disconnect("Hacked client").unwrap();
            }
        }
    }

    /// Mods and plugins can use this to send their data.
    /// Minecraft's internal channels are prefixed with MC|.
    fn handle_plugin_message(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // TODO: Do something
        let channel = rbuf.read_string().unwrap();
        debug!("Channel: {}", channel);
        let mut data = Vec::new();
        rbuf.read_to_end(&mut data).unwrap();
    }

    pub fn keep_alive(&mut self, id: i32) {
        if self.state != State::Play {
            return;
        }

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap(); // Keep Alive packet
        wbuf.write_var_int(id).unwrap(); // Keep Alive ID

        if let Err(e) = self.write_packet(&wbuf) {
            if Protocol::is_disconnection_error(e.kind()) {
                self.state = State::Disconnected;
            }
        }
    }

    fn join_game(&mut self, player: Arc<RwLock<Player>>, world: Arc<RwLock<World>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap(); // Join Game packet

        {
            let p = player.read().unwrap();
            {
                let client_lock = p.client();
                let c = client_lock.read().unwrap();
                wbuf.write_int(c.id() as i32).unwrap(); // The player's Entity ID
            }
            wbuf.write_ubyte(p.gamemode() as u8).unwrap(); // Gamemode
        }
        {
            let w = world.read().unwrap();
            wbuf.write_byte(w.dimension() as i8).unwrap(); // Dimension
        }

        wbuf.write_ubyte(self.server.difficulty() as u8).unwrap(); // Difficulty
        wbuf.write_ubyte(self.server.max_players() as u8).unwrap(); // Max players
        wbuf.write_string(self.server.level_type()).unwrap(); // Level Type? (default, flat, largeBiomes, amplified, default_1_1)
        wbuf.write_bool(false).unwrap(); // Reduced debug info?

        self.write_packet(&wbuf)
    }

    fn chat_message(&mut self, raw_msg: String) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x02).unwrap(); // Chat Message packet

        // TODO:
        wbuf.write_string(&format!("{{ \"text\": \"{}\" }}", raw_msg))
            .unwrap(); // JSON Data
        wbuf.write_ubyte(0).unwrap(); // Position: 0: chat (chat box), 1: system message (chat box), 2: above hotbar

        self.write_packet(&wbuf)
    }

    fn time_update(&mut self, _world: Arc<RwLock<World>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x03).unwrap(); // Time Update packet

        // TODO: write actual values
        wbuf.write_long(0).unwrap(); // World Age
        wbuf.write_long(0).unwrap(); // Time of day

        self.write_packet(&wbuf)
    }

    fn spawn_position(&mut self, world: Arc<RwLock<World>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x05).unwrap(); // Spawn Position packet

        let spawn_pos = world.read().unwrap().spawn_pos();
        wbuf.write_position(spawn_pos.x, spawn_pos.y, spawn_pos.z)
            .unwrap(); // Spawn location

        self.write_packet(&wbuf)
    }

    fn player_pos_look(&mut self, player: Arc<RwLock<Player>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x08).unwrap(); // Player Position And Look packet

        {
            let p = player.read().unwrap();
            let pos = p.pos();
            wbuf.write_double(pos.x).unwrap(); // X
            wbuf.write_double(pos.y).unwrap(); // y
            wbuf.write_double(pos.z).unwrap(); // z
            wbuf.write_float(p.yaw()).unwrap(); // Yaw
            wbuf.write_float(p.pitch()).unwrap(); // Pitch
            wbuf.write_byte(0).unwrap(); // Flags
        }

        self.write_packet(&wbuf)
    }

    fn spawn_player(&mut self, player: Arc<RwLock<Player>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x0C).unwrap(); // Player Spawn packet

        {
            let p = player.read().unwrap();
            {
                let client_lock = p.client();
                let c = client_lock.read().unwrap();
                wbuf.write_var_int(c.id() as i32).unwrap(); // The player's Entity ID

                wbuf.write_all(c.uuid().as_bytes()).unwrap();
            }

            let pos = p.pos();
            wbuf.write_int((pos.x * 32f64) as i32).unwrap();
            wbuf.write_int((pos.y * 32f64) as i32).unwrap();
            wbuf.write_int((pos.z * 32f64) as i32).unwrap();

            wbuf.write_byte(p.yaw() as i8).unwrap();
            wbuf.write_byte(p.pitch() as i8).unwrap();

            wbuf.write_short(0).unwrap();

            wbuf.write_ubyte(0).unwrap();
            wbuf.write_ubyte(0).unwrap();

            wbuf.write_ubyte(0x82).unwrap();
            {
                let client_lock = p.client();
                let c = client_lock.read().unwrap();
                wbuf.write_string(c.get_username().unwrap()).unwrap();
            }

            wbuf.write_ubyte(0x66).unwrap();
            wbuf.write_float(p.health()).unwrap();

            wbuf.write_ubyte(0x0A).unwrap();
            wbuf.write_ubyte(p.skin_parts().bits()).unwrap();

            wbuf.write_ubyte(0x7f).unwrap();
        }

        self.write_packet(&wbuf)
    }

    /// Chunks are not unloaded by the client automatically.
    /// To unload chunks, send this packet with Ground-Up Continuous=true and no 16^3 chunks (eg. Primary Bit Mask=0).
    /// The server does not send skylight information for nether-chunks,
    /// it's up to the client to know if the player is currently in the nether.
    /// You can also infer this information from the primary bitmask and the amount of uncompressed bytes sent.
    fn chunk_data(&mut self, coord: ChunkCoord, chunk_map: Arc<ChunkMap>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x21).unwrap(); // Chunk Data packet

        // TODO: write actual values
        wbuf.write_int(coord.x).unwrap(); // Chunk X
        wbuf.write_int(coord.z).unwrap(); // Chunk Z

        // This is true if the packet represents all sections in this vertical column,
        // where the Primary Bit Mask specifies exactly which sections are included, and which are air
        wbuf.write_bool(true).unwrap(); // Ground-Up Continuous

        chunk_map.do_with_chunk(coord, |chunk: &Chunk| {
            let bit_mask = chunk.data.get_primary_bit_mask();
            wbuf.write_ushort(bit_mask).unwrap(); // Primary Bit Mask

            chunk.serialize(&mut wbuf).unwrap();
        });

        self.write_packet(&wbuf)
    }

    /// https://wiki.vg/index.php?title=Protocol&oldid=7368#Change_Game_State
    fn change_game_state(&mut self, reason: GameStateReason, value: f32) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x2B).unwrap(); // Change Game State packet

        wbuf.write_ubyte(reason as u8).unwrap(); // Reason
        wbuf.write_float(value).unwrap(); // Value

        self.write_packet(&wbuf)
    }

    fn player_list_item(
        &mut self,
        action: PlayerListAction,
        players: Box<[Arc<RwLock<Player>>]>,
    ) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x38).unwrap(); // Player List Item packet

        wbuf.write_var_int(action as i32).unwrap(); // Action
        wbuf.write_var_int(players.len() as i32).unwrap(); // Number Of Players

        for player in players.iter() {
            let player = player.read().unwrap();
            let client = player.client();
            let client = client.read().unwrap();

            wbuf.write_all(client.uuid().as_bytes()).unwrap(); // UUID

            match action {
                PlayerListAction::AddPlayer => {
                    wbuf.write_string(client.get_username().unwrap()).unwrap();
                    match client.properties().as_array() {
                        Some(properties) => {
                            wbuf.write_var_int(properties.len() as i32).unwrap();
                            for prop in properties {
                                wbuf.write_string(prop["name"].as_str().unwrap()).unwrap();
                                wbuf.write_string(prop["value"].as_str().unwrap()).unwrap();
                                let signature = prop.get("signature");
                                wbuf.write_bool(signature.is_some()).unwrap();
                                if let Some(signature) = signature {
                                    wbuf.write_string(signature.as_str().unwrap()).unwrap();
                                }
                            }
                        }
                        _ => wbuf.write_var_int(0).unwrap(),
                    }

                    wbuf.write_var_int(player.gamemode() as i32).unwrap(); // Gamemode

                    // TODO: calculate actual ping
                    wbuf.write_var_int(250).unwrap(); // Ping

                    wbuf.write_bool(false).unwrap(); // Has Display Name
                }
                PlayerListAction::UpdateGamemode => {
                    wbuf.write_var_int(player.gamemode() as i32).unwrap()
                } // Gamemode
                PlayerListAction::UpdateLatency => wbuf.write_var_int(250).unwrap(), // Ping
                PlayerListAction::UpdateDisplayName => wbuf.write_bool(false).unwrap(), // Has Display Name,
                PlayerListAction::RemovePlayer => (),
            }
        }

        self.write_packet(&wbuf)
    }

    fn player_abilities(&mut self, player: Arc<RwLock<Player>>) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x39).unwrap(); // Player Abilities packet

        {
            let p = player.read().unwrap();
            wbuf.write_ubyte(p.abilities().bits()).unwrap();
        }

        wbuf.write_float(0.05 * 1.0).unwrap(); // Flying Speed
        // Modifies the field of view, like a speed potion.
        // A Notchian server will use the same value as the movement speed
        wbuf.write_float(0.1 * 1.0).unwrap(); // Field of View Modifier

        self.write_packet(&wbuf)
    }

    /// Changes the difficulty setting in the client's option menu
    fn server_difficulty(&mut self, difficulty: Difficulty) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x41).unwrap(); // Server Difficulty packet

        wbuf.write_ubyte(difficulty as u8).unwrap(); // Difficulty

        self.write_packet(&wbuf)
    }

    fn resource_pack_send(&mut self, url: &str, hash: &str) -> Result<()> {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x48).unwrap(); // Resource Pack Send packet

        wbuf.write_string(url).unwrap(); // URL
        wbuf.write_string(hash).unwrap(); // Hash

        self.write_packet(&wbuf)
    }

    // Other packets:
    fn disconnect(&mut self, reason: &str) -> Result<()> {
        debug_assert!(self.state == State::Login || self.state == State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(match self.state {
            State::Login => 0x00,
            State::Play => 0x40,
            _ => panic!("Unknown state for Disconnect Packet: {:?}", self.state),
        })?; // Disconnect packet

        info!("Kicking with reason: '{}'", reason);

        let reason = json!({
            "text": reason
        });
        wbuf.write_string(&reason.to_string())?;
        self.write_packet(&wbuf)?;
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<()> {
        self.state = State::Disconnected;
        self.stream.shutdown(Shutdown::Both)?;
        Ok(())
    }

    fn is_disconnection_error(e: ErrorKind) -> bool {
        e == ErrorKind::NotConnected
            || e == ErrorKind::ConnectionAborted
            || e == ErrorKind::ConnectionRefused
            || e == ErrorKind::ConnectionReset
            || e == ErrorKind::BrokenPipe
    }
}

impl Drop for Protocol {
    fn drop(&mut self) {
        self.server.remove_client(self.client_id);
    }
}
