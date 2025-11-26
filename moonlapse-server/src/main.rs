use std::env;

use env_logger::Builder;
use log::LevelFilter;
use tokio::sync::mpsc;

mod messages;

use crate::{game::{Game, GameOptions}, messages::HubMessage, net::hub::{Hub, HubOptions}};

mod net;
mod game;
mod utils;

// can easily swap out serializers
#[macro_export]
macro_rules! serialize {
    ($expression:expr) => {
        moonlapse_shared::serializers::json::serialize($expression)
    };
}

#[macro_export]
macro_rules! deserialize {
    ($expression:expr) => {
        moonlapse_shared::serializers::json::deserialize($expression)
    };
}

#[tokio::main]
async fn main() {
    let mut port = 42523u16;

    let args: Vec<String> = env::args().collect();
    if let [_, port_str] = &args[..] {
        if let Ok(p) = port_str.parse::<u16>() {
            port = p;
        }
    }

    // setup logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    let (hub_to_game_tx, hub_to_game_rx) = mpsc::channel::<HubMessage>(1024);
    let (game_to_hub_tx, game_to_hub_rx) = mpsc::channel::<HubMessage>(1024);

    // spawn game task
    let mut game = Game::new(GameOptions{tick_rate: 20}, (game_to_hub_tx.clone(), hub_to_game_rx));
    tokio::spawn(async move { game.start().await; });

    let mut hub = Hub::new(HubOptions{ port }, (hub_to_game_tx, game_to_hub_rx));
    hub.start().await;
}
