pub mod messages;
pub mod components;

use std::{collections::HashMap, sync::Arc, time::Duration};

use hecs::{Entity, World};
use log::{info, warn};
use tokio::{sync::{Mutex, mpsc}, time};

use crate::{game::{components::{Movable, Player, Position}, messages::GameMessage}, net::packets::{Component, Direction, Packet}};

pub struct GameOptions {
    pub tick_rate: u8
}

pub struct Game {
    opts: GameOptions,
    hub_tx: mpsc::UnboundedSender<GameMessage>,
    hub_rx: mpsc::UnboundedReceiver<GameMessage>,
    world: World,
    /// Mapping of player connection_id -> in-game Entity
    conn_entity_map: HashMap<u64, Entity>
}

impl Game {
    pub fn new(opts: GameOptions, hub_tx: mpsc::UnboundedSender<GameMessage>, hub_rx: mpsc::UnboundedReceiver<GameMessage>) -> Game {

        // register all systems

        Game{opts, hub_tx, hub_rx, world: World::new(), conn_entity_map: HashMap::new()}
    }

    pub async fn start(&mut self) {
        
        // start tick loop
        let mut ticks = 0u64;
        let ideal_tick_duration = 1000 / self.opts.tick_rate as u64;
        loop {
            let start_time = time::Instant::now();

            // tick!
            self.tick(&ticks).await;

            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            
            if elapsed_ms >= ideal_tick_duration {
                warn!("Tick time budget exceeded. Goal time: {} ms, actual time: {} ms", ideal_tick_duration, elapsed_ms);
            } else {
                let remaining_sleep_time = ideal_tick_duration - elapsed_ms;
                time::sleep(Duration::from_millis(remaining_sleep_time)).await;
            }
            ticks += 1;
        }
    }

    async fn tick(&mut self, ticks: &u64) {
        // read all incoming messages from hub
        while let Some(msg) = self.hub_rx.recv().await {
            match msg {
                GameMessage::PacketFromClient(conn_id, packet) => {
                    match packet {
                        Packet::Translate(dir) => {
                            let (dx, dy) = match dir {
                                Direction::Up => (0, 1),
                                Direction::Down => (0, -1),
                                Direction::Right => (1, 0),
                                Direction::Left => (-1, 0)
                            };
                            self.translate_player(conn_id, dx, dy);
                        },
                        _ => {}
                    }
                },
                GameMessage::ClientDisconnected(conn_id) => {
                    // TODO: clean up actual entity stuff
                    if let Some(entity) = self.conn_entity_map.remove(&conn_id) {
                        _ = self.world.despawn(entity);
                    }
                },
                GameMessage::ClientConnected(conn_id) => {
                    let bundle = (Player{id: conn_id}, Movable{}, Position{x: 0, y: 0});
                    let entity = self.world.spawn(bundle);
                    self.conn_entity_map.insert(conn_id, entity);
                    
                    // TODO: make this nicer!
                    let _ = self.hub_tx.send(GameMessage::Broadcast(
                        Packet::ComponentUpdate(entity.id(), Component::Position(Position{x: 0, y: 0}))
                    ));
                    let _ = self.hub_tx.send(GameMessage::Broadcast(
                        Packet::ComponentUpdate(entity.id(), Component::Player(Player{id: conn_id}))
                    ));

                }
                _ => warn!("Unhandled message")
            }
        }
    }

    fn translate_player(&mut self, conn_id: u64, dx: i8, dy: i8) {
        if let Some(&entity) = self.conn_entity_map.get(&conn_id) {
            if self.world.get::<&Movable>(entity).is_ok() {
                if let Ok(mut pos) = self.world.get::<&mut Position>(entity) {
                    pos.x += dx as i32;
                    pos.y += dy as i32;

                    let _ = self.hub_tx.send(GameMessage::Broadcast(
                        Packet::ComponentUpdate(entity.id(), Component::Position(Position{x: pos.x, y: pos.y})),
                    ));
                }
            } else {
                warn!("Entity for conn_id {} is not movable", conn_id);
            }
        } else {
            warn!("Unknown connection id {}", conn_id);
        }
    }
}