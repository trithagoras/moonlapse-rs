use anyhow::Result;

use crate::packets::Packet;

pub fn serialize(p: &Packet) -> Result<Vec<u8>> {
    let data = rmp_serde::to_vec(&p)?;
    Ok(data)
}

pub fn deserialize(data: &[u8]) -> Result<Packet> {
    let p: Packet = rmp_serde::from_slice(&data)?;
    Ok(p)
}
