use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Player { pub id: u64 }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position { pub x: i32, pub y: i32 }

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Whether the entity is allowed to move
pub struct Movable {}