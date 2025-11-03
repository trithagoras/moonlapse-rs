use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packet {
    Id(u64),
    Ok,
    Deny,
    PlayerDisconnected(u64),
    Register(String, String),
    Login(String, String)
}