use hecs::{Entity, World};
use log::{warn};
use moonlapse_shared::{components::*, packets::{Component, Direction}};
use tokio::sync::mpsc;

use crate::messages::GameMessage;

pub fn movement_system(world: &mut World, game_tx: &mpsc::Sender<GameMessage>) {
    // for all entities with Position + Velocity
    let mut to_broadcast = Vec::new();
    for (entity, (pos, vel)) in world.query_mut::<(&mut Position, &mut Velocity)>() {
        // filter out entities where velocity is unchanged since last tick
        if (vel.dx, vel.dy) == (0, 0) {
            continue;
        }

        // TODO: collisions
        pos.x += vel.dx;
        pos.y += vel.dy;

        // reset velocity to 0
        vel.dx = 0;
        vel.dy = 0;

        // queue up an update packet
        to_broadcast.push((entity.id(), Position { x: pos.x, y: pos.y }));
    }

    // broadcast all updated positions
    for (eid, pos) in to_broadcast {
        let _ = game_tx.blocking_send(GameMessage::ComponentUpdate(eid, Component::Position(pos)));
    }
}

pub fn set_entity_velocity(world: &mut World, entity: &Entity, dir: Direction) {
    let (dx, dy) = match dir {
        Direction::Up => (0, 1),
        Direction::Down => (0, -1),
        Direction::Right => (1, 0),
        Direction::Left => (-1, 0)
    };
    // is entity movable?
    if world.get::<&Velocity>(*entity).is_ok() {
        if let Ok(mut vel) = world.get::<&mut Velocity>(*entity) {
            vel.dx += dx;
            vel.dy += dy;
        } else {
            warn!("Attempted to apply velocity to unmovable entity {}", entity.id());
        }
    }
}
