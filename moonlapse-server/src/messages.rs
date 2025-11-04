use moonlapse_shared::packets::{Packet};

/// Messages to be sent between the Hub and a Connection
#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    /// Signals from Connection to Hub to say "i've disconnected"
    Disconnected(u64),
    /// Signals from Connection that a packet has been received
    PacketReceived(u64, Packet),
    /// Signals from Hub to send a packet to the client
    SendPacket(Packet)
}


#[derive(Debug, Clone)]
/// Messages to be sent between the Hub and the Game layers.
/// assists in translation between connection concepts and game concepts
pub enum HubMessage {
    PacketFromClient(u64, Packet),
    ClientDisconnected(u64),
    ClientConnected(u64),
    SendTo(u64, Packet),
    Broadcast(Packet)
}
