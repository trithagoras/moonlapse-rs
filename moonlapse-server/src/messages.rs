use moonlapse_shared::{ConnId, EntityId, packets::{Component, Packet}};

/// Messages to be sent between the Hub and a Connection
#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    /// Signals from Connection to Hub to say "i've disconnected"
    Disconnected(ConnId),
    /// Signals from Connection that a packet has been received
    PacketReceived(ConnId, Packet),
    /// Signals from Hub to send a packet to the client
    SendPacket(Packet)
}


#[derive(Debug, Clone)]
/// Messages to be sent between the Hub and the Game layers.
/// assists in translation between connection concepts and game concepts
pub enum HubMessage {
    PacketFromClient(ConnId, Packet),
    // Not necessarily connection closed; just that player logged out
    PlayerLeft(ConnId),
    /// Player logged in. (conn_id, username)
    PlayerJoined(ConnId, String),
    SendTo(ConnId, Packet),
    Broadcast(Packet)
}

#[derive(Debug, Clone)]
/// Messages to be sent within the Game layer
/// e.g. from System to System
pub enum GameMessage {
    ComponentUpdate(EntityId, Component)
}