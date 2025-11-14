use serde::{Deserialize, Serialize};

use crate::{ConnId, EntityId, WorldSnapshot, components::{Player, Position}};

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
    Id(ConnId),
    Ok,
    /// Deny with a reason. Usually used in form validation, e.g. login/registration form.
    Deny(String),
    /// Telling a client about who has disconnected
    PlayerDisconnected(ConnId),
    Register(String, String),
    Login(String, String),
    /// A chat from the client to the server
    Chat(String),
    /// Denotes a chat from another player (id)
    ChatBroadcast(ConnId, String),
    /// Client wants to move in a direction
    Translate(Direction),
    ComponentUpdate(EntityId, Component),
    WorldSnapshot(WorldSnapshot)
}