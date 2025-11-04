use env_logger::Builder;
use log::LevelFilter;
use tokio::sync::mpsc;

use crate::{game::{Game, GameOptions, messages::GameMessage}, net::hub::{Hub, HubOptions}};

mod net;
mod game;

// can easily swap out serializers
#[macro_export]
macro_rules! serialize {
    ($expression:expr) => {
        crate::net::serializers::msgpack::serialize($expression)
    };
}

#[macro_export]
macro_rules! deserialize {
    ($expression:expr) => {
        crate::net::serializers::msgpack::deserialize($expression)
    };
}

#[tokio::main]
async fn main() {
    // setup logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    let (hub_to_game_tx, hub_to_game_rx) = mpsc::unbounded_channel::<GameMessage>();
    let (game_to_hub_tx, game_to_hub_rx) = mpsc::unbounded_channel::<GameMessage>();

    // spawn game task
    let mut game = Game::new(GameOptions{tick_rate: 20}, game_to_hub_tx.clone(), hub_to_game_rx);
    tokio::spawn(async move { game.start().await; });

    let mut hub = Hub::new(HubOptions{ port: 42523 }, hub_to_game_tx, game_to_hub_rx);
    hub.start().await;
}
