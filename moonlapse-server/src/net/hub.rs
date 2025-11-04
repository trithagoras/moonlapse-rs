use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use log::{debug, error, info};
use tokio::{net::TcpListener, sync::{Mutex, broadcast, mpsc}};

use crate::{messages::{ConnectionMessage, HubMessage}, net::{connection::Connection}};
use moonlapse_shared::packets::Packet;


#[derive(Debug)]
enum SendTo {
    Id(u64),
    All
}

pub struct HubOptions {
    pub port: u16,
}

pub struct Hub {
    opts: HubOptions,
    hub_tx: mpsc::UnboundedSender<ConnectionMessage>,
    hub_rx: mpsc::UnboundedReceiver<ConnectionMessage>,
    game_tx: mpsc::UnboundedSender<HubMessage>,
    game_rx: mpsc::UnboundedReceiver<HubMessage>,
    broadcast_tx: broadcast::Sender<ConnectionMessage>,
    connections: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<ConnectionMessage>>>>
}

impl Hub {
    pub fn new(opts: HubOptions, game_tx: mpsc::UnboundedSender<HubMessage>, game_rx: mpsc::UnboundedReceiver<HubMessage>) -> Hub {
        let (hub_tx, hub_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(500);
        Hub { opts, hub_tx, hub_rx, broadcast_tx, connections: Arc::new(Mutex::new(HashMap::new())), game_tx, game_rx }
    }

    pub async fn start(&mut self) {

        // fire off listen_loop
        let connections = self.connections.clone();
        let port = self.opts.port;
        let hub_tx = self.hub_tx.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let game_tx = self.game_tx.clone();
        tokio::spawn(async move {
            Self::listen_loop(port, connections, hub_tx, broadcast_tx, game_tx).await?;
            Ok::<_, anyhow::Error>(())
        });

        // event reactive router
        loop {
            tokio::select! {
                Some(msg) = self.hub_rx.recv() => {
                    self.handle_connection_message(msg).await;
                }

                Some(msg) = self.game_rx.recv() => {
                    self.handle_game_message(msg).await;
                }

                else => break, // all senders dropped
            }
        }
    }

    async fn handle_connection_message(&self, msg: ConnectionMessage) {
        match msg {
            ConnectionMessage::PacketReceived(id, packet) => match packet {
                Packet::Chat(s) => {
                    // broadcast the received chat to all connections
                    self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::ChatBroadcast(id, s))).await;
                }
                p => {
                    // just forward all other packets directly to the game to handle
                    let _ = self.game_tx.send(HubMessage::PacketFromClient(id, p));
                }
            },
            ConnectionMessage::Disconnected(id) => {
                // forward to game to clean up. Nothing else required here since other cleanup is handled already.
                let _ = self.game_tx.send(HubMessage::ClientDisconnected(id));
                // broadcast a disconnected packet to all other players
                self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::PlayerDisconnected(id))).await;
            }
            _ => {}
        }
    }

    async fn handle_game_message(&self, msg: HubMessage) {
        match msg {
            HubMessage::SendTo(id, pkt) => {
                self.send_msg(SendTo::Id(id), ConnectionMessage::SendPacket(pkt)).await;
            }
            HubMessage::Broadcast(pkt) => {
                self.send_msg(SendTo::All, ConnectionMessage::SendPacket(pkt)).await;
            }
            _ => {}
        }
    }

    /// Listens for new connections and dispatches them
    async fn listen_loop(port: u16, connections: Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<ConnectionMessage>>>>, hub_tx: mpsc::UnboundedSender<ConnectionMessage>, broadcast_tx: broadcast::Sender<ConnectionMessage>, game_tx: mpsc::UnboundedSender<HubMessage>) -> Result<()> {
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

            // send connection message to game
            _ = game_tx.send(HubMessage::ClientConnected(conn_id));

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