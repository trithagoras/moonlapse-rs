use crate::net::packets::Packet;


#[derive(Debug, Clone)]
pub enum GameMessage {
    PacketFromClient(u64, Packet),
    ClientDisconnected(u64),
    ClientConnected(u64),
    SendTo(u64, Packet),
    Broadcast(Packet)
}