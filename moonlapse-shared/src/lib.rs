use serde::{Deserialize, Serialize};

use crate::components::*;

pub mod components;
pub mod serializers;
pub mod packets;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorldSnapshot {
    pub players: Vec<(u32, Player)>,
    pub positions: Vec<(u32, Position)>,
}