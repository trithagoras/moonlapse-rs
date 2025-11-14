use serde::{Deserialize, Serialize};

use crate::components::*;

pub mod components;
pub mod serializers;
pub mod packets;

pub type ConnId = u64;
pub type EntityId = u32;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    pub players: Vec<(EntityId, Player)>,
    pub positions: Vec<(EntityId, Position)>,
}