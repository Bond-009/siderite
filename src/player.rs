use protocol::Protocol;

pub struct Player {
    connection: Protocol
}

impl Player {

    pub fn new(connection: Protocol) -> Player {
        Player {
            connection: connection
        }
    }
}
