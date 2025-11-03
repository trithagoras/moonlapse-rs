use std::sync::{Arc};

use anyhow::Result;
use log::{error, info};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, sync::{Mutex, broadcast, mpsc}};

use crate::{packets::Packet, serializer::{deserialize, serialize}};

/// Messages to be sent between the Hub and a Connection
#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    /// Signals from Hub to drop the connection
    Disconnect,
    /// Signals from Hub to send all packets in the outbox
    SendPackets,
    /// Signals from Connection that a packet has been received
    PacketReceived(u64, Packet),
    /// Signals from Hub to enqueue a packet into the outbox
    EnqueuePacket(Packet)
}

/// Object in charge of communication with a single client.
/// Handles reading all incoming data, deserialization,
/// sending incoming packets back to the hub, and
/// sending any packets from the outbox back to the client.
pub struct Connection {
    id: u64,
    /// messages coming directly from the Hub
    hub_rx: mpsc::UnboundedReceiver<ConnectionMessage>,
    /// messages to send directly to the Hub
    hub_tx: mpsc::UnboundedSender<ConnectionMessage>,
    /// all packets to send to client on next tick
    outbox: Arc<Mutex<Vec<Packet>>>,
    socket: TcpStream,
    broadcast_rx: broadcast::Receiver<ConnectionMessage>
}

impl Connection {
    pub fn new(id: u64, hub_rx: mpsc::UnboundedReceiver<ConnectionMessage>, hub_tx: mpsc::UnboundedSender<ConnectionMessage>, socket: TcpStream, broadcast_rx: broadcast::Receiver<ConnectionMessage>) -> Connection {
        Connection { id, hub_rx, hub_tx, outbox: Arc::new(Mutex::new(Vec::new())), socket, broadcast_rx }
    }

    pub async fn start(mut self) -> Result<()> {
        // split socket into read and write halves
        let (mut reader, mut writer) = tokio::io::split(self.socket);

        info!("New connection: {}", self.id);

        // enqueue an initial Id packet for client
        {
            let mut lock = self.outbox.lock().await;
            lock.push(Packet::Id(self.id));
        }

        // task to read packets from client
        let hub_tx = self.hub_tx.clone();
        let socket_read = async move {
            let mut buf = vec![0; 1024];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    info!("Client {} disconnected", self.id);
                    break;
                }

                match deserialize(&buf[..n]) {
                    Err(e) => error!("Error deserializing from {}: {}", self.id, e),
                    Ok(p) => {
                        hub_tx.send(ConnectionMessage::PacketReceived(self.id, p))?; // TODO: probably don't kill connection...
                    }
                };
            }
            Ok::<_, anyhow::Error>(())
        };

        // task to read messages from hub
        let outbox = self.outbox.clone();
        let hub_read = async move {
            'outer: loop {
                while let Some(msg) = self.hub_rx.recv().await {
                    match msg {
                        ConnectionMessage::Disconnect => {
                            // TODO: is 'break' enough?
                            break 'outer;
                        },
                        ConnectionMessage::EnqueuePacket(p) => {
                            let mut lock = outbox.lock().await;
                            lock.push(p);
                        },
                        _ => {}
                    }
                }
            }
            Ok::<_, anyhow::Error>(())
        };

        // task to read broadcasted messages
        let outbox = self.outbox.clone();
        let broadcast_read = async move {
            'outer: loop {
                while let Ok(msg) = self.broadcast_rx.recv().await {
                    match msg {
                        ConnectionMessage::Disconnect => {
                            // TODO: is 'break' enough?
                            break 'outer;
                        },
                        ConnectionMessage::EnqueuePacket(p) => {
                            let mut lock = outbox.lock().await;
                            lock.push(p);
                        },
                        ConnectionMessage::SendPackets => {
                            let lock = outbox.lock().await;
                            for p in lock.iter() {
                                info!("Sending packet {:?} to connection {}", p, self.id);
                                let data = serialize(&p)?;
                                writer.write(&data).await?;
                            }
                        },
                        _ => {}
                    }
                }
            }
            Ok::<_, anyhow::Error>(())
        };

        tokio::select! {
            _ = socket_read => {}
            _ = hub_read => {}
            _ = broadcast_read => {}
        }

        Ok(())
    }

}