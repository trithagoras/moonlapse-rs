pub mod systems;

use std::{collections::HashMap, time::Duration};

use hecs::{Entity, World};
use log::{warn};
use moonlapse_shared::{WorldSnapshot, components::{Player, Position, Velocity}, packets::{Component, Packet}};
use tokio::{sync::mpsc, time};

use crate::{game::systems::{movement_system, set_entity_velocity}, messages::HubMessage};

pub struct GameOptions {
    pub tick_rate: u8
}

pub struct Game {
    opts: GameOptions,
    hub_tx: mpsc::UnboundedSender<HubMessage>,
    hub_rx: mpsc::UnboundedReceiver<HubMessage>,
    world: World,
    /// Mapping of player connection_id -> in-game Entity
    conn_entity_map: HashMap<u64, Entity>
}

impl Game {
    pub fn new(opts: GameOptions, hub_tx: mpsc::UnboundedSender<HubMessage>, hub_rx: mpsc::UnboundedReceiver<HubMessage>) -> Game {
        Game{opts, hub_tx, hub_rx, world: World::new(), conn_entity_map: HashMap::new()}
    }

    pub async fn start(&mut self) {
        
        // start tick loop
        let mut ticks = 0u64;
        let ideal_tick_duration = 1000 / self.opts.tick_rate as u64;
        loop {
            let start_time = time::Instant::now();

            // tick!
            self.tick(&ticks);

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

    fn tick(&mut self, _ticks: &u64) {
        // dispatch all incoming messages from hub
        while let Ok(msg) = self.hub_rx.try_recv() {
            self.dispatch_hub_message(msg);
        }

        self.run_systems();
    }

    fn run_systems(&mut self) {
        movement_system(&mut self.world, &self.hub_tx);
    }

    fn dispatch_hub_message(&mut self, msg: HubMessage) {
        match msg {
            HubMessage::PacketFromClient(conn_id, packet) => {
                match packet {
                    Packet::Translate(dir) => {
                        if let Some(entity) = self.conn_entity_map.get(&conn_id) {
                            set_entity_velocity(&mut self.world, &entity, dir);
                        }
                    },
                    _ => {}
                }
            },
            HubMessage::ClientDisconnected(conn_id) => {
                // TODO: clean up actual entity stuff
                if let Some(entity) = self.conn_entity_map.remove(&conn_id) {
                    _ = self.world.despawn(entity);
                }
            },
            HubMessage::ClientConnected(conn_id) => {
                let bundle = (Player{id: conn_id}, Velocity{dx: 0, dy: 0}, Position{x: 0, y: 0});
                let entity = self.world.spawn(bundle);
                self.conn_entity_map.insert(conn_id, entity);

                // TODO: broadcasts and regular sends are sent in indeterminable order due to select! in Connection
                // that's probably bad....

                // send a current world snapshot to the new connection
                let snapshot = Self::create_world_snapshot(&self.world);
                let _ = self.hub_tx.send(HubMessage::SendTo(conn_id, Packet::WorldSnapshot(snapshot)));
                
                // broadcast our new connection to everyone else
                // TODO: make this nicer!
                // e.g. ComponentUpdate((c1, c2, ...))
                let _ = self.hub_tx.send(HubMessage::Broadcast(
                    Packet::ComponentUpdate(entity.id(), Component::Position(Position{x: 0, y: 0}))
                ));
                let _ = self.hub_tx.send(HubMessage::Broadcast(
                    Packet::ComponentUpdate(entity.id(), Component::Player(Player{id: conn_id}))
                ));

            }
            _ => warn!("Unhandled message")
        }
    }

    /// Creates a snapshot of the world to be sent to new connections.
    fn create_world_snapshot(world: &World) -> WorldSnapshot {
        let mut players = Vec::new();
        let mut positions = Vec::new();

        for (entity, player) in world.query::<&Player>().iter() {
            players.push((entity.id(), player.clone()));
        }

        for (entity, pos) in world.query::<&Position>().iter() {
            positions.push((entity.id(), pos.clone()));
        }

        WorldSnapshot { players, positions }
    }
}