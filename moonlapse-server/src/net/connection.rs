use anyhow::Result;
use log::{debug, error, info};
use tokio::{io::{AsyncReadExt, AsyncWriteExt, WriteHalf}, net::TcpStream};

use crate::{deserialize, messages::ConnectionMessage, serialize, utils::TxRx};
use moonlapse_shared::{ConnId, packets::Packet};

/// Object in charge of communication with a single client.
/// Handles reading all incoming data, deserialization,
/// sending incoming packets back to the hub, and
/// sending any packets from the outbox back to the client.
pub struct Connection {
    id: ConnId,
    hub_txrx: TxRx<ConnectionMessage>,
    socket: TcpStream,
}

impl Connection {
    pub fn new(id: ConnId, hub_txrx: TxRx<ConnectionMessage>, socket: TcpStream) -> Connection {
        Connection { id, hub_txrx, socket}
    }

    async fn send_packet(writer: &mut WriteHalf<TcpStream>, p: Packet, id: ConnId) {
        debug!("Sending packet {:?} to connection {}", p, id);
        let res = serialize!(&p);
        if let Err(ref e) = res {
            error!("Error serializing packet {:?} to send to client: {} - {:?}", &p, id, &e);
            return;
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
        let hub_tx = self.hub_txrx.0.clone();
        let socket_read = async move {
            let mut buf = vec![0; 1024];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    info!("Client ID={} requested disconnection", id);
                    break;
                }

                match deserialize!(&buf[..n]) {
                    Ok(p) => {
                        if let Err(e) = hub_tx.send(ConnectionMessage::PacketReceived(id, p)).await {
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
                let msg = self.hub_txrx.1.recv().await;
                if let Some(ConnectionMessage::SendPacket(p)) = msg {
                    Self::send_packet(&mut writer, p, id).await;
                } else {
                    // TODO: What here??
                }
            }
        };


        tokio::select! {
            _ = socket_read => {}
            _ = hub_read => {}
        }
        Ok(())
    }

}