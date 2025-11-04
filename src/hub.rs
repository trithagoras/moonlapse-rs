use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use log::{debug, error, info, warn};
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
        let (broadcast_tx, _) = broadcast::channel(500);
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

    pub async fn tick(&mut self, ticks: &u64) {
        // dispatch incoming messages from all connections
        while let Ok(msg) = self.hub_rx.try_recv() {
            match msg {
                ConnectionMessage::PacketReceived(id, p) => {
                    debug!("Packet received from connection {}: {:?}", id, p);
                    // DEBUG: just sending back an Ok packet
                    self.send_msg(SendTo::Id(id), ConnectionMessage::SendPacket(Packet::Ok)).await;
                },
                ConnectionMessage::Disconnected(id) => {
                    // broadcast to everyone
                    self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::PlayerDisconnected(id))).await
                }
                _ => {}
            }
        }

        // DEBUG:
        // send an Ok packet every second
        if *ticks % 20 == 0 {
            self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::Ok)).await;
        }

        // DEBUG:
        // disconnect everyone after 15 seconds
        if *ticks % 300 == 0 {
            self.send_msg(SendTo::All, ConnectionMessage::Disconnect).await;
        }
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

            let conns = connections.clone();
            let hub_tx = hub_tx.clone();
            let broadcast_rx = broadcast_tx.subscribe();
            let conn = Connection::new(conn_id, conn_rx, hub_tx.clone(), broadcast_rx, stream);
            tokio::spawn(async move {
                conn.start().await?;
                
                // connection finished
                let mut lock = conns.lock().await;
                lock.remove(&conn_id);
                debug!("Cleaned up after connection {}", conn_id);
                
                hub_tx.send(ConnectionMessage::Disconnected(conn_id))?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    async fn send_msg_to(&self, id: u64, msg: &ConnectionMessage) {
        let lock = self.connections.lock().await;
        if let None = lock.get(&id) {
            error!("Tried to send message to non-existent client: {}", id);
            return;
        }
        let tx = lock[&id].clone();
        match tx.send(msg.clone()) {
            Err(e) => error!("Error sending message {:?} to connection {} - {}", msg, id, e),
            _ => {}
        }
    }

    async fn send_msg(&self, to: SendTo, msg: ConnectionMessage) {
        match to {
            SendTo::Id(id) => self.send_msg_to(id, &msg).await,
            SendTo::All => {
                match self.broadcast_tx.send(msg.clone()) {
                    Err(_) => {},   // an error usually indicates that there are no current subscribers.
                    Ok(n) => debug!("Broadcasted message {:?} to {} connections", msg, n)
                };
            }
        }
    }
}