use serde::{Deserialize, Serialize};

use crate::game::{WorldSnapshot, components::{Player, Position}};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Component {
    Position(Position),
    Player(Player)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packet {
    Id(u64),
    Ok,
    Deny,
    /// Telling a client about who has disconnected
    PlayerDisconnected(u64),
    Register(String, String),
    Login(String, String),
    /// A chat from the client to the server
    Chat(String),
    /// Denotes a chat from another player (id)
    ChatBroadcast(u64, String),
    /// Client wants to move in a direction
    Translate(Direction),
    ComponentUpdate(u32, Component),
    WorldSnapshot(WorldSnapshot)
}