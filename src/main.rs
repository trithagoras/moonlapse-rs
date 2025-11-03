use env_logger::Builder;
use log::LevelFilter;

use crate::hub::{Hub, HubOptions};

mod packets;
mod hub;
mod serializer;
mod connection;

#[tokio::main]
async fn main() {
    // setup logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    let mut hub = Hub::new(HubOptions{ tick_rate: 20, port: 42523, max_connections: 100 });
    hub.start().await;
}
