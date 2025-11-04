use anyhow::Result;

use crate::packets::Packet;

pub fn serialize(p: &Packet) -> Result<Vec<u8>> {
    let data = serde_json::to_vec(&p)?;
    Ok(data)
}

pub fn deserialize(data: &[u8]) -> Result<Packet> {
    let p: Packet = serde_json::from_slice(&data)?;
    Ok(p)
}
