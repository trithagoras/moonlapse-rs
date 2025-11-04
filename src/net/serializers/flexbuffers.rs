use flexbuffers::{FlexbufferSerializer, Reader};
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::net::packets::Packet;

pub fn serialize(p: &Packet) -> Result<Vec<u8>> {
    let mut s = FlexbufferSerializer::new();
    p.serialize(&mut s)?;
    Ok(s.take_buffer())
}

pub fn deserialize(data: &[u8]) -> Result<Packet> {
    let r = Reader::get_root(data)?;
    let packet: Packet = Packet::deserialize(r)?;
    Ok(packet)
}
