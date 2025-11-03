use env_logger::Builder;
use log::LevelFilter;

use crate::hub::{Hub, HubOptions};

mod packets;
mod hub;
mod serializers;
mod connection;

// can easily swap out serializers
#[macro_export]
macro_rules! serialize {
    ($expression:expr) => {
        crate::serializers::msgpack::serialize($expression)
    };
}

#[macro_export]
macro_rules! deserialize {
    ($expression:expr) => {
        crate::serializers::msgpack::deserialize($expression)
    };
}

#[tokio::main]
async fn main() {
    // setup logger
    Builder::new()
        .filter_level(LevelFilter::Info)
        .init();

    

    let mut hub = Hub::new(HubOptions{ tick_rate: 20, port: 42523 });
    hub.start().await;
}
