use anyhow::Result;
use log::{error, info};
use tokio::{io::{AsyncReadExt, AsyncWriteExt, WriteHalf}, net::{TcpStream}, sync::mpsc};

use crate::{packets::Packet, serializer::{deserialize, serialize}};

/// Messages to be sent between the Hub and a Connection
#[derive(Debug, Clone)]
pub enum ConnectionMessage {
    /// Signals from Hub to drop the connection
    Disconnect,
    /// Signals from Connection to Hub to say "i've disconnected"
    Disconnected(u64),
    /// Signals from Connection that a packet has been received
    PacketReceived(u64, Packet),
    /// Signals from Hub to send a packet to the client
    SendPacket(Packet)
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
    socket: TcpStream
}

impl Connection {
    pub fn new(id: u64, hub_rx: mpsc::UnboundedReceiver<ConnectionMessage>, hub_tx: mpsc::UnboundedSender<ConnectionMessage>, socket: TcpStream) -> Connection {
        Connection { id, hub_rx, hub_tx, socket }
    }

    async fn send_packet(writer: &mut WriteHalf<TcpStream>, p: Packet, id: u64) {
        info!("Sending packet {:?} to connection {}", p, id);
        let res = serialize(&p);
        if let Err(ref e) = res {
            error!("Error serializing packet {:?} to send to client: {} - {:?}", &p, id, &e);
        }
        let data = res.unwrap();
        let res = writer.write(&data).await;
        if let Err(ref e) = res {
            error!("Error writing packet {:?} to client: {} - {:?}", &p, id, &e);
        }
    }

    pub async fn start(mut self) -> Result<()> {
        // split socket into read and write halves
        let (mut reader, mut writer) = tokio::io::split(self.socket);

        let id = self.id;

        info!("New connection: {}", self.id);

        // send an initial Id packet for client
        Self::send_packet(&mut writer, Packet::Id(self.id), self.id).await;

        // task to read packets from client
        let hub_tx = self.hub_tx.clone();
        let socket_read = async move {
            let mut buf = vec![0; 1024];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    info!("Client {} disconnected", id);
                    break;
                }

                match deserialize(&buf[..n]) {
                    Ok(p) => {
                        if let Err(e) = hub_tx.send(ConnectionMessage::PacketReceived(id, p)) {
                            error!("Failed to send PacketReceived for {}: {}", id, e);
                            break;
                        }
                    }
                    Err(e) => error!("Error deserializing data from {}: {}", id, e),
                }
            }
            Ok::<_, anyhow::Error>(())
        };

        // task to read messages from hub
        let hub_read = async move {
            loop {
                let msg = self.hub_rx.recv().await;
                match msg {
                    None => {
                        info!("Hub channel closed for {}", id);
                        break;
                    }
                    Some(ConnectionMessage::Disconnect) => {
                        info!("Hub requested disconnect for {}", id);
                        break;
                    }
                    Some(ConnectionMessage::SendPacket(p)) => {
                        Self::send_packet(&mut writer, p, id).await;
                    }
                    _ => {}
                }
            }
            Ok::<_, anyhow::Error>(())
        };


        tokio::select! {
            _ = socket_read => {}
            _ = hub_read => {}
        }
        Ok(())
    }

}