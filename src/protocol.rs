use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};

use nbt::{ReadNBTExt, WriteNBTExt};

#[derive(Copy, Clone, PartialEq)]
enum State {
    HandSchaking = 0x00,
    Status = 0x01,
    Login = 0x02,
    Game = 0x03
}

pub struct Protocol {
    stream: TcpStream,
    state: State
}

impl Protocol {
    pub fn new(stream: TcpStream) -> Protocol {
        Protocol {
            stream: stream,
            state: State::HandSchaking
        }
    }

    pub fn data_received(&mut self) {

        // FIXME: Pls fix
        let mut tbuf: [u8; 1] = [0; 1];
        self.stream.peek(&mut tbuf).unwrap();
        let first_byte = tbuf[0];
        println!("fisrt byte: {:X}", first_byte);
        if first_byte == 0xFE {
            self.handle_legacy_ping();
            return;
        }
        drop(tbuf);
        drop(first_byte);

        loop {
            let lenght = match self.stream.read_var_int() {
                Ok(value) => value,
                //Err(_error) => continue 
                Err(error) =>  {
                    if error.kind() == ErrorKind::UnexpectedEof {
                        println!("Connection terminated");
                        break;
                    }
                    else {
                        continue; // Not enough data
                    }
                }
            };

            let mut rbuf = vec![0u8; lenght as usize];

            println!("lenght {}", lenght);

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
        println!("Legacy ping");
        let payload = self.stream.read_ubyte().unwrap();
        println!("{:X}", payload);
    }

    fn handle_packet(&mut self, rbuf: &[u8], id: i32) {
        println!("Packet id: {:#X}", id);
        match self.state {
            State::HandSchaking => {
                if id != 0x00 {
                    panic!("Unknown packet ");
                }
                self.handle_handschake(rbuf);
            }
            State::Status => {
                match id {
                    0x00 => self.handle_request(),
                    0x01 => self.handle_ping(rbuf),
                    _ => panic!("Unknown packet ")
                }
            }
            State::Login => {
                unimplemented!("No login packets implemented");
            }
            State::Game => {
                unimplemented!("No game packets implemented");
            }
        }
    }

    fn handle_handschake(&mut self, mut rbuf: &[u8]) {
        let _proto_v = rbuf.read_var_int().unwrap();
        let _server_address = rbuf.read_string().unwrap();
        let _server_port = rbuf.read_ushort().unwrap();
        if rbuf.read_var_int().unwrap() != 1 {
            panic!("I don't know");
        }
        self.state = State::Status;
    }


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
                "max": 10,
                "online": 1,
                "sample": [
                    {
                        "name": "thinkofdeath",
                        "id": "4566e69f-c907-48ee-8d71-d7ba5aa00d20"
                    }
                ]
            },
            "description": {
                "text": "Siderite custom Minecraft server"
            }
            //"favicon": "data:image/png;base64,"
        });
        let strres = response.to_string();
        println!("{}", strres);
        wbuf.write_string(&strres).unwrap();
        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    fn handle_ping(&mut self, mut rbuf: &[u8]) {
        let mut wbuf = Vec::new();
        let payload = rbuf.read_long().unwrap();
        println!("Payload: {}", payload);
        wbuf.write_var_int(0x01).unwrap();
        wbuf.write_long(payload).unwrap();
        Protocol::write_packet(&mut self.stream, &wbuf);
    }

    fn write_packet(mut stream: &mut TcpStream, mut rbuf: &[u8]) {
        for &byte in rbuf {
            print!("{:X} ", byte);
        }
        let lenght = rbuf.len() as i32;
        println!("\nwbuf len {}, id: {:#X}", lenght, rbuf.first().unwrap());
        stream.write_var_int(lenght).unwrap(); // Write packet lenght
        stream.write_all(&mut rbuf).unwrap();
    }
}
