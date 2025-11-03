use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use log::{error, info, warn};
use tokio::{net::TcpListener, sync::{Mutex, broadcast, mpsc}, time};

use crate::{connection::{Connection, ConnectionMessage}, packets::Packet};


#[derive(Debug)]
enum SendTo {
    Id(u64),
    All
}

pub struct HubOptions {
    pub tick_rate: u8,
    pub port: u16,
    pub max_connections: usize
}

pub struct Hub {
    opts: HubOptions,
    hub_tx: mpsc::UnboundedSender<ConnectionMessage>,
    hub_rx: mpsc::UnboundedReceiver<ConnectionMessage>,
    broadcast_tx: broadcast::Sender<ConnectionMessage>,
    connections: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<ConnectionMessage>>>>
}

impl Hub {
    pub fn new(opts: HubOptions) -> Hub {
        let (hub_tx, hub_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(opts.max_connections);
        Hub { opts, hub_tx, hub_rx, broadcast_tx, connections: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub async fn start(&mut self) {

        // fire off listen_loop
        let connections = self.connections.clone();
        let port = self.opts.port;
        let hub_tx = self.hub_tx.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        tokio::spawn(async move {
            Self::listen_loop(port, connections, hub_tx, broadcast_tx).await?;
            Ok::<_, anyhow::Error>(())
        });

        // start tick loop
        let ideal_tick_duration = 1000 / self.opts.tick_rate as u64;
        loop {
            let start_time = time::Instant::now();

            // tick!
            self.tick().await;

            let elapsed_ms = start_time.elapsed().as_millis() as u64;
            
            if elapsed_ms >= ideal_tick_duration {
                warn!("Tick time budget exceeded. Goal time: {} ms, actual time: {} ms", ideal_tick_duration, elapsed_ms);
            } else {
                let remaining_sleep_time = ideal_tick_duration - elapsed_ms;
                time::sleep(Duration::from_millis(remaining_sleep_time)).await;
            }
        }
    }

    pub async fn tick(&mut self) {
        // read all incoming messages from all connections
        while let Ok(msg) = self.hub_rx.try_recv() {
            match msg {
                ConnectionMessage::PacketReceived(id, p) => {
                    info!("Packet received from some {}: {:?}", id, p);
                    // DEBUG: just sending back an Ok packet
                    self.send_msg(SendTo::Id(id), ConnectionMessage::EnqueuePacket(Packet::Ok)).await;
                },
                _ => {}
            }
        }

        self.send_msg(SendTo::All, ConnectionMessage::SendPackets).await;
    }

    /// Listens for new connections and dispatches them
    async fn listen_loop(port: u16, connections: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<ConnectionMessage>>>>, hub_tx: mpsc::UnboundedSender<ConnectionMessage>, broadcast_tx: broadcast::Sender<ConnectionMessage>) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        info!("Hub listening for connections on port {}", port);

        let mut conn_id = 0u64;

        loop {
            let res = listener.accept().await;
            if let Err(e) = res {
                error!("Error accepting new connection: {}", e);
                continue;
            }
            let (stream, _) = res.unwrap();
            conn_id += 1;

            // create new connection object, assign ID, spawn off task
            // connection channels
            let (conn_tx, conn_rx) = mpsc::unbounded_channel();
            {
                let mut lock = connections.lock().await;
                lock.insert(conn_id, conn_tx);
            }

            let hub_tx = hub_tx.clone();
            let conn = Connection::new(conn_id, conn_rx, hub_tx, stream, broadcast_tx.subscribe());
            tokio::spawn(async move {
                conn.start().await?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    async fn send_msg(&self, to: SendTo, msg: ConnectionMessage) {
        if let SendTo::All = to {
            match self.broadcast_tx.send(msg.clone()) {
                Err(e) => error!("Error sending message {:?} to connection {:?} - {}", msg, to, e),
                _ => {}
            }
            return;
        }

        if let SendTo::Id(id) = to {
            let lock = self.connections.lock().await;
            if let None = lock.get(&id) {
                error!("Tried to send message to non-existent client: {}", id);
                return;
            }
            let tx = lock[&id].clone();
            match tx.send(msg.clone()) {
                Err(e) => error!("Error sending message {:?} to connection {:?} - {}", msg, to, e),
                _ => {}
            }
        }
    }
}