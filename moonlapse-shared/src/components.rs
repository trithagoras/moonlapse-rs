use serde::{Deserialize, Serialize};

use crate::ConnId;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Player { pub id: ConnId }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityDetails { pub name: String, pub description: String }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Position { pub x: i32, pub y: i32 }

#[derive(Debug, Serialize, Deserialize, Clone)]
/// Used in conjunction with the Position component in the MovementSystem
pub struct Velocity { pub dx: i32, pub dy: i32 }