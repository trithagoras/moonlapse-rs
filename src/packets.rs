use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packet {
    Id(u64),
    Ok,
    Deny,
    Register(String, String),
    Login(String, String)
}