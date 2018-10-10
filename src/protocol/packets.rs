pub enum Packet {
    // Login state
    /// UUID, Username
    LoginSuccess(),
    // Other
    /// Reason
    Disconnect(String),
}
