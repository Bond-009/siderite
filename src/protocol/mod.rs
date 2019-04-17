pub mod authenticator;
pub mod packets;
pub mod thread;
pub mod v47;

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, RwLock, mpsc};
use std::sync::mpsc::{Receiver, Sender};

use circbuf::CircBuf;
use openssl::rsa::Padding;
use openssl::sha;
use openssl::symm::{encrypt, decrypt, Cipher};
use num_traits::FromPrimitive;
use rand::{thread_rng, Rng};
use uuid::adapter::Hyphenated;

use self::authenticator::AuthInfo;
use self::packets::Packet;

use crate::client::Client;
use crate::entities::player::{GameMode, Player};
use crate::mc_ext::{MCReadExt, MCWriteExt};
use crate::server::Server;
use crate::storage::world::{Difficulty, World};

const VERIFY_TOKEN_LEN: usize = 4;
const ENCRYPTION_KEY_LEN: usize = 16;

#[repr(i32)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
enum State {
    HandSchaking = 0x00,
    Status = 0x01,
    Login = 0x02,
    Play = 0x03,
    Disconnected = 0xFF
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, FromPrimitive, PartialEq)]
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

pub struct Protocol {
    server: Arc<Server>,
    client: Arc<RwLock<Client>>,
    authenticator: Sender<AuthInfo>,
    receiver: Receiver<Packet>,

    stream: TcpStream,
    state: State,
    received_data: CircBuf,

    encrypted: bool,
    verify_token: [u8; VERIFY_TOKEN_LEN],
    cipher: Cipher,
    encryption_key: [u8; ENCRYPTION_KEY_LEN]
}

impl Protocol {

    pub fn new(client_id: i32, server: Arc<Server>, stream: TcpStream, authenticator: Sender<AuthInfo>) -> Protocol {
        let mut arr = [0u8; VERIFY_TOKEN_LEN];
        thread_rng().fill(&mut arr[..]);
        let (tx, rx) = mpsc::channel();
        Protocol {
            server: server.clone(),
            client: Arc::new(RwLock::new(Client::new(client_id, server, tx))), // TODO: proper client id
            receiver: rx,
            authenticator: authenticator,

            stream: stream,
            state: State::HandSchaking,
            received_data: CircBuf::with_capacity(32 * 1024).unwrap(),

            encrypted: false,
            verify_token: arr,
            cipher: Cipher::aes_128_cfb8(), // AES/CFB8 stream cipher.
            encryption_key: [0u8; ENCRYPTION_KEY_LEN]
        }
    }

    pub fn get_client(&self) -> Arc<RwLock<Client>> {
        self.client.clone()
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
        stream.peek(&mut tbuf).unwrap();
        if tbuf[0] == 0xFE {
            stream.read(&mut tbuf).unwrap();
            Protocol::handle_legacy_ping(&mut stream);
            stream.shutdown(Shutdown::Both).expect("shutdown call failed");
            return true;
        }
        return false;
    }

    fn handle_legacy_ping(stream: &mut TcpStream) {
        let payload = stream.read_ubyte().unwrap();
        assert_eq!(payload, 1);
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
            Err(e) => panic!("Encountered IO error: {}", e),
        };
        drop(tmp);

        if len == 0 {
            // Connection closed
            self.state = State::Disconnected;
            return;
        }

        let mut vec = vec![0u8; len];
        self.stream.read(&mut vec).unwrap();

        if self.encrypted {
            vec = decrypt(self.cipher, &self.encryption_key, Some(&self.encryption_key), &vec).unwrap();
        }
        self.received_data.write(&vec).unwrap();
        self.handle_in_packets();
    }

    fn handle_in_packets(&mut self) {
        loop {
            if self.received_data.len() == 0 {
                // No data
                return;
            }

            let lenght = match self.received_data.read_var_int() {
                Ok(value) => value as usize,
                Err(_) => {
                    debug!("Not enough data");
                    return; // Not enough data
                }
            };

            debug!("Packet lenght: {}", lenght);

            if self.received_data.len() < lenght {
                return; // Not enough data
            }

            let mut rbuf = vec![0u8; lenght];
            self.received_data.read(&mut rbuf).unwrap();

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
                    _ => self.unknown_packet(id, rbuf)
                }
            }
            State::Status => {
                match id {
                    0x00 => self.handle_request(),
                    0x01 => self.handle_ping(rbuf),
                    _ => self.unknown_packet(id, rbuf)
                }
            }
            State::Login => {
                match id {
                    0x00 => self.handle_login_start(rbuf),
                    0x01 => self.handle_encryption_response(rbuf),
                    _ => self.unknown_packet(id, rbuf)
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
                    0x10 => self.handle_creative_inventory_action(rbuf),
                    0x13 => self.handle_player_abilities(rbuf),
                    0x15 => self.handle_client_settings(rbuf),
                    0x16 => self.handle_client_status(rbuf),
                    0x17 => self.handle_plugin_message(rbuf),
                    _ => self.unknown_packet(id, rbuf)
                }
            }
            State::Disconnected => return // Ignore all packets
        }
    }

    fn unknown_packet(&mut self, id: i32, _rbuf: &[u8]) {
        error!("Unknown packet: {:#X}, state: {:?}", id, self.state);
        self.disconnect(&format!("Unknown packet: {:#X}", id));
    }

    // Out:

    pub fn handle_out_packets(&mut self) {
        let mut packets = Vec::new();
        for p in self.receiver.try_iter() {
            packets.push(p);
        }
        for p in packets {
            self.send_packet(p);
        }
    }

    fn send_packet(&mut self, packet: Packet) {
        match packet {
            Packet::LoginSuccess()                 => self.login_success(),

            Packet::JoinGame(player, world)        => self.join_game(player, world),
            Packet::TimeUpdate(world)              => self.time_update(world),
            Packet::SpawnPosition(world)           => self.spawn_position(world),
            Packet::PlayerPositionAndLook(player)  => self.player_pos_look(player),
            Packet::PlayerAbilities(player)        => self.player_abilities(player),
            Packet::ChunkData(x, z, primary_bit_mask, data) => self.chunk_data(x, z, primary_bit_mask, &data),
            Packet::ServerDifficulty()             => self.server_difficulty(Difficulty::Normal), // TODO: change
            Packet::ChangeGameState(reason, value) => self.change_game_state(reason, value),

            Packet::Disconnect(reason)             => self.disconnect(&reason)
        }
    }

    fn write_packet(&mut self, mut rbuf: &[u8]) {
        let lenght = rbuf.len();
        debug!("Write packet: state: {:?}, len {}, id: {:#X}", self.state, lenght, rbuf.first().unwrap());
        let mut vec = Vec::with_capacity(lenght + 4);
        vec.write_var_int(lenght as i32).unwrap(); // Write packet lenght
        vec.write_all(&mut rbuf).unwrap(); // Write packet data

        // TODO: don't encrypt per packet
        if self.encrypted {
            vec = encrypt(self.cipher, &self.encryption_key, Some(&self.encryption_key), &vec).unwrap();
        }
        self.stream.write(&mut vec).unwrap();
    }

    // HandSchaking packets:

    fn handle_handschake(&mut self, mut rbuf: &[u8]) {
        let proto_v = rbuf.read_var_int().unwrap();
        assert_eq!(proto_v, 47);
        let _server_address = rbuf.read_string().unwrap();
        let _server_port = rbuf.read_ushort().unwrap();
        let next_state = rbuf.read_var_int().unwrap();
        self.state = State::from_i32(next_state).unwrap();
        debug!("Changed State to {:?}", self.state);
    }

    // Status packets:

    fn handle_request(&mut self) {
        assert_eq!(self.state, State::Status);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x00).unwrap();
        // TODO: Add favicon and sample
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
            },
            //"favicon": "data:image/png;base64,"
        });
        let strres = response.to_string();
        wbuf.write_string(&strres).unwrap();
        self.write_packet(&wbuf);
    }

    fn handle_ping(&mut self, mut rbuf: &[u8]) {
        assert_eq!(self.state, State::Status);

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

        if self.server.authenticate {
            self.encryption_request();

            self.client.write().unwrap().username = Some(username);
        }
        else {
            self.authenticator.send(AuthInfo {
                client_id: self.client.read().unwrap().id,
                username: username,
                server_id: None
            }).unwrap();
        }
    }

    fn handle_encryption_response(&mut self, mut rbuf: &[u8]) {
        let ss_len = rbuf.read_var_int().unwrap() as usize; // Shared Secret Key Length
        debug!("ss_len {}", ss_len);
        let mut ssarr = vec![0u8; ss_len];
        rbuf.read(&mut ssarr).unwrap(); // Shared Secret
        
        let vt_len = rbuf.read_var_int().unwrap() as usize; // Verify Token Length
        debug!("vt_len {}", vt_len);
        let mut vtarr = vec![0u8; vt_len];
        rbuf.read(&mut vtarr).unwrap(); // Verify Token

        // Decrypt the and verify the Verify Token
        let mut vtdvec = vec![0; vt_len];
        let vtd_len = self.server.private_key.private_decrypt(&vtarr, &mut vtdvec, Padding::PKCS1).unwrap();
        if vtd_len != VERIFY_TOKEN_LEN {
            debug!("Verify Token is the wrong length: expected {}, got {}", VERIFY_TOKEN_LEN, vtd_len);
            self.disconnect("Hacked client");
            return;
        }

        if &vtdvec[..VERIFY_TOKEN_LEN] != &self.verify_token[..] {
            debug!("Verify Token is not the same");
            self.disconnect("Hacked client");
            return;
        }

        // Decrypt Shared Secret Key
        let mut ssdvec = vec![0; ss_len];
        let ssd_len = self.server.private_key.private_decrypt(&ssarr, &mut ssdvec, Padding::PKCS1).unwrap();
        if ssd_len != ENCRYPTION_KEY_LEN {
            debug!("Shared Secret Key is the wrong length: expected {}, got {}", ENCRYPTION_KEY_LEN, ssd_len);
            self.disconnect("Hacked client");
            return;
        }

        // Enables AES/CFB8 encryption
        self.encryption_key.copy_from_slice(&ssdvec[..ENCRYPTION_KEY_LEN]);
        self.encrypted = true;

        let mut hasher = sha::Sha1::new();
        hasher.update(self.server.id.as_bytes());
        hasher.update(&self.encryption_key);
        hasher.update(&self.server.public_key_der);
        let hash = hasher.finish();
        let server_id = authenticator::java_hex_digest(hash);

        let client = self.client.write().unwrap();
        let username = client.username.clone().unwrap();

        self.authenticator.send(AuthInfo {
                client_id: client.id,
                username: username,
                server_id: Some(server_id)
            }).unwrap();
    }

    fn encryption_request(&mut self) {
        assert_eq!(self.state, State::Login);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap(); // Encryption Request packet
        wbuf.write_string(&self.server.id).unwrap();
        // Public Key
        wbuf.write_var_int(self.server.public_key_der.len() as i32).unwrap();
        wbuf.write_all(&self.server.public_key_der).unwrap();
        // Verify Token
        wbuf.write_var_int(self.verify_token.len() as i32).unwrap();
        wbuf.write_all(&self.verify_token).unwrap();

        self.write_packet(&wbuf);
    }

    fn login_success(&mut self) {
        assert_eq!(self.state, State::Login);

        // TODO: option to enable compression

        self.state = State::Play;
        debug!("Changed State to {:?}", self.state);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x02).unwrap(); // Login Success packet

        {
            let client = self.client.read().unwrap();

            let uuid = client.get_uuid().unwrap();
            let uuid_str: String = Hyphenated::from_uuid(uuid).to_string();
            debug!("uuid: {}", uuid_str);
            debug!("name: {}", client.username.clone().unwrap());

            wbuf.write_string(&uuid_str).unwrap();
            wbuf.write_string(&client.username.clone().unwrap()).unwrap();
        }

        self.write_packet(&wbuf);
    }

    fn _login_set_compression(&mut self) {
        assert_eq!(self.state, State::Login);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x03).unwrap(); // Login Success packet

        // Maximum size of a packet before its compressed 
        wbuf.write_var_int(256).unwrap(); // Threshold

        self.write_packet(&wbuf);
    }

    // Play packets:

    /// The server will frequently send out a keep-alive, each containing a random ID.
    /// The client must respond with the same packet.
    fn handle_keep_alive(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _id = rbuf.read_var_int().unwrap();
        // TODO: Keep alive
    }

    /// Check the message to see if it begins with a '/'.
    /// If it does, the server assumes it to be a command and attempts to process it.
    /// If it doesn't, the username of the sender is prepended and sent to all clients.
    fn handle_chat_message(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let msg = rbuf.read_string().unwrap();
        if msg.starts_with('/') {
            // Exec cmd
        }
        info!("{}", msg);
    }

    /// This packet is used to indicate whether the player is on ground (walking/swimming),
    /// or airborne (jumping/falling).
    fn handle_player(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let on_ground = rbuf.read_bool().unwrap();
        debug!("On Ground: {}", on_ground);
    }

    /// Updates the player's XYZ position on the server.
    fn handle_player_pos(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // Feet pos
        let x = rbuf.read_double().unwrap();
        let y = rbuf.read_double().unwrap();
        let z = rbuf.read_double().unwrap();
        debug!("Feet pos: ({}, {}, {})", x, y, z);
        let on_ground = rbuf.read_bool().unwrap();
        debug!("On Ground: {}", on_ground);
    }

    /// Updates the direction the player is looking in.
    fn handle_player_look(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let _yaw = rbuf.read_float().unwrap();
        let _pitch = rbuf.read_float().unwrap();
        let on_ground = rbuf.read_bool().unwrap();
        debug!("On Ground: {}", on_ground);
    }

    /// A combination of Player Look and Player Position.
    fn handle_player_pos_look(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // TODO: Do something
        // Feet pos
        let x = rbuf.read_double().unwrap();
        let y = rbuf.read_double().unwrap();
        let z = rbuf.read_double().unwrap();
        debug!("Feet pos: ({}, {}, {})", x, y, z);
        let _yaw = rbuf.read_float().unwrap();
        let _pitch = rbuf.read_float().unwrap();
        let on_ground = rbuf.read_bool().unwrap();
        debug!("On Ground: {}", on_ground);
    }

    /// Sent when the player mines a block. A Notchian server only accepts
    /// digging packets with coordinates within a 6-unit radius of the player's position.
    fn handle_player_digging(&mut self, mut rbuf: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        // Value | Meaning
        // --------------
        // 0     | Started digging
        // 1     | Cancelled digging
        // 2     | Finished digging
        // 3     | Drop item stack
        // 4     | Drop item
        // 5     | Shoot arrow / finish eating
        let _status = rbuf.read_byte().unwrap();
        let (_x, _y, _z) = rbuf.read_position().unwrap();

        // Value | Offset
        // --------------
        // 0     | -Y
        // 1     | +Y
        // 2     | -Z
        // 3     | +Z
        // 4     | -X
        // 5     | +X
        let face = rbuf.read_byte().unwrap();
        debug_assert!(face >= 0 && face < 6);
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

        let _window_id = rbuf.read_ubyte().unwrap();
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

        // Bit  | Meaning
        // --------------------
        // 0x01 | is Creative
        // 0x02 | is flying
        // 0x04 | can fly
        // 0x08 | damage disabled (god mode)
        let flags = rbuf.read_byte().unwrap();
        let _is_creative = (flags & 0x01) == 0x01;
        let _is_flying = (flags & 0x02) == 0x02;
        let _can_fly = (flags & 0x04) == 0x04;
        let _god_mode = (flags & 0x08) == 0x08;
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
        let _skin_parts = rbuf.read_ubyte().unwrap();
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
                self.disconnect("Hacked client");
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

    fn join_game(&mut self, player: Arc<RwLock<Player>>, world: Arc<RwLock<World>>) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x01).unwrap(); // Join Game packet

        // TODO:
        wbuf.write_int(0).unwrap(); // The player's Entity ID
        {
            let p = player.read().unwrap();
            wbuf.write_ubyte(p.get_gamemode() as u8).unwrap(); // Gamemode
        }
        {
            let w = world.read().unwrap();
            wbuf.write_byte(w.get_dimension() as i8).unwrap(); // Dimension
            wbuf.write_ubyte(w.get_difficulty() as u8).unwrap(); // Difficulty
        }
        let max_players = self.server.max_players;

        wbuf.write_ubyte(max_players as u8).unwrap(); // Max players
        wbuf.write_string(&"default").unwrap(); // Level Type? (default, flat, largeBiomes, amplified, default_1_1)
        wbuf.write_bool(false).unwrap(); // Reduced debug info?

        self.write_packet(&wbuf);
    }

    fn time_update(&mut self, _world: Arc<RwLock<World>>) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x03).unwrap(); // Time Update packet

        // TODO: write actual values
        wbuf.write_long(0).unwrap(); // World Age
        wbuf.write_long(0).unwrap(); // Time of day

        self.write_packet(&wbuf);
    }

    fn spawn_position(&mut self, _world: Arc<RwLock<World>>) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x05).unwrap(); // Spawn Position packet

        // TODO: Write world spawn
        wbuf.write_position(10, 65, 10).unwrap(); // Spawn location

        self.write_packet(&wbuf);
    }

    fn player_pos_look(&mut self, _player: Arc<RwLock<Player>>) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x08).unwrap(); // Player Position And Look packet

        // TODO: write actual values
        wbuf.write_double(10.0).unwrap(); // X
        wbuf.write_double(65.0).unwrap(); // y
        wbuf.write_double(10.0).unwrap(); // z
        wbuf.write_float(10.0).unwrap(); // Yaw
        wbuf.write_float(0.0).unwrap(); // Pitch
        wbuf.write_byte(0).unwrap(); // Flags

        self.write_packet(&wbuf);
    }

    /// Chunks are not unloaded by the client automatically.
    /// To unload chunks, send this packet with Ground-Up Continuous=true and no 16^3 chunks (eg. Primary Bit Mask=0).
    /// The server does not send skylight information for nether-chunks,
    /// it's up to the client to know if the player is currently in the nether.
    /// You can also infer this information from the primary bitmask and the amount of uncompressed bytes sent.
    fn chunk_data(&mut self, x: i32, z: i32, primary_bit_mask: u16, data: &[u8]) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x21).unwrap(); // Chunk Data packet

        // TODO: write actual values
        wbuf.write_int(x).unwrap(); // Chunk X
        wbuf.write_int(z).unwrap(); // Chunk Z

        // This is true if the packet represents all sections in this vertical column,
        // where the Primary Bit Mask specifies exactly which sections are included, and which are air 
        wbuf.write_bool(true).unwrap(); // Ground-Up Continuous
        
        wbuf.write_ushort(primary_bit_mask).unwrap(); // Primary Bit Mask

        wbuf.write_var_int(data.len() as i32).unwrap();

        wbuf.write_all(data).unwrap();

        self.write_packet(&wbuf);
    }

    fn player_abilities(&mut self, player: Arc<RwLock<Player>>) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x39).unwrap(); // Player Abilities packet

        // Field         | Bit
        // --------------|-----
        // Invulnerable  | 0x01
        // Flying        | 0x02
        // Allow Flying  | 0x04
        // Creative Mode | 0x08
        {
            let p = player.read().unwrap();
            let mut flags: i8 = 0;
            if p.get_gamemode() == GameMode::Creative {
                flags |= 0x08; // Creative Mode
            }
            // TODO: use actual values
            flags |= 0x01; // Invulnerable
            flags |= 0x02; // Flying
            flags |= 0x04; // Allow Flying

            wbuf.write_byte(flags).unwrap();
        }

        wbuf.write_float(0.05 * 1.0).unwrap(); // Flying Speed
        // Modifies the field of view, like a speed potion.
        // A Notchian server will use the same value as the movement speed
        wbuf.write_float(0.1 * 1.0).unwrap(); // Field of View Modifier

        self.write_packet(&wbuf);
    }

    /// Changes the difficulty setting in the client's option menu
    fn server_difficulty(&mut self, difficulty: Difficulty) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x41).unwrap(); // Server Difficulty packet

        wbuf.write_ubyte(difficulty as u8).unwrap(); // Difficulty

        self.write_packet(&wbuf);
    }


    /// https://wiki.vg/index.php?title=Protocol&oldid=7368#Change_Game_State
    fn change_game_state(&mut self, reason: GameStateReason, value: f32) {
        debug_assert_eq!(self.state, State::Play);

        let mut wbuf = Vec::new();
        wbuf.write_var_int(0x2B).unwrap(); // Change Game State packet

        wbuf.write_ubyte(reason as u8).unwrap(); // Reason
        wbuf.write_float(value).unwrap(); // Value

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

        info!("Kicking with reason: '{}'", reason);

        let reason = json!({
            "text": reason
        });
        wbuf.write_string(&reason.to_string()).unwrap();
        self.write_packet(&wbuf);
        let _ = self.stream.shutdown(Shutdown::Both);
        self.state = State::Disconnected;
    }
}
