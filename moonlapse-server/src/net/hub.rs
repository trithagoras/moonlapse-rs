use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow};
use log::{debug, error, info};
use tokio::{net::TcpListener, sync::{Mutex, broadcast, mpsc}};

use crate::{messages::{ConnectionMessage, HubMessage}, net::connection::{Connection}};
use moonlapse_shared::{ConnId, packets::Packet};

#[derive(Debug)]
enum SendTo {
    Id(ConnId),
    All,
    Matches(fn (ConnectionDetails) -> bool)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    /// Before logging in
    Entry,
    /// After logging in
    LoggedIn
}

#[derive(Clone, Copy, Debug)]
pub struct ConnectionDetails {
    pub id: ConnId,
    pub state: ConnectionState
}

#[derive(Clone, Debug)]
struct ConnectionWrapper {
    conn_tx: mpsc::UnboundedSender<ConnectionMessage>,
    details: ConnectionDetails
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
    connections: Arc<Mutex<HashMap<ConnId, ConnectionWrapper>>>
}

impl Hub {
    pub fn new(opts: HubOptions, game_tx: mpsc::UnboundedSender<HubMessage>, game_rx: mpsc::UnboundedReceiver<HubMessage>) -> Hub {
        let (hub_tx, hub_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, _) = broadcast::channel(500);
        Hub { opts, hub_tx, hub_rx, broadcast_tx, connections: Arc::new(Mutex::new(HashMap::new())), game_tx, game_rx }
    }

    async fn get_conn(&self, id: ConnId) -> Result<ConnectionWrapper> {
        let lock = self.connections.lock().await;
        return match lock.get(&id) {
            None => Err(anyhow!("Attempted to get connection for unregistered ID: {}", id)),
            Some(conn) => Ok(conn.clone())
        };
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
                    match self.handle_connection_message(msg.clone()).await {
                        Err(e) => error!("Error handling incoming message {:?} from a connection: {}",  msg, e),
                        _ => {}
                    }
                }

                Some(msg) = self.game_rx.recv() => {
                    match self.handle_game_message(msg.clone()).await {
                        Err(e) => error!("Error handling incoming message {:?} from game: {}",  msg, e),
                        _ => {}
                    }
                }

                else => break, // all senders dropped
            }
        }
    }

    async fn handle_connection_game_message(&self, msg: ConnectionMessage) -> Result<()> {
        match msg {
            ConnectionMessage::PacketReceived(id, packet) => match packet {
                Packet::Chat(s) => {
                    // broadcast the received chat to all connections
                    self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::ChatBroadcast(id, s))).await?;
                }
                p => {
                    // just forward all other packets directly to the game to handle
                    let _ = self.game_tx.send(HubMessage::PacketFromClient(id, p));
                }
            },
            ConnectionMessage::Disconnected(id) => {
                // forward to game to clean up. Nothing else required here since other cleanup is handled already.
                let _ = self.game_tx.send(HubMessage::PlayerLeft(id));
                // broadcast a disconnected packet to all other players
                self.send_msg(SendTo::All, ConnectionMessage::SendPacket(Packet::PlayerDisconnected(id))).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_connection_entry_message(&self, msg: ConnectionMessage) -> Result<()> {
        match msg {
            ConnectionMessage::PacketReceived(id, packet) => match packet {
                Packet::Register(username, password) => {

                }
                Packet::Login(username, password) => {
                    // DEBUG USER:
                    if username == "jon" {
                        if password == "123" {
                            // success! set state to game
                            {
                                let mut lock = self.connections.lock().await;
                                if let Some(entry) = lock.get_mut(&id) {
                                    entry.details.state = ConnectionState::LoggedIn;
                                }
                            }
                            // send message to game to init entity, etc.
                            _ = self.game_tx.send(HubMessage::PlayerJoined(id, username));
                        } else {
                            self.send_msg_to(id, ConnectionMessage::SendPacket(Packet::Deny("Incorrect password".into()))).await?;
                        }
                    } else {
                        self.send_msg_to(id, ConnectionMessage::SendPacket(Packet::Deny("User does not exist".into()))).await?;
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    async fn handle_connection_message(&self, msg: ConnectionMessage) -> Result<()> {
        // TODO: is this cooked?
        if let ConnectionMessage::Disconnected(_) = msg {
            // if disconnected message, just broadcast it.
            // don't check if the id is in connections because it probably isn't,
            // due to bad async timing
            self.send_msg(SendTo::Matches(|det| det.state == ConnectionState::LoggedIn), msg.clone()).await?;
            return Ok(());
        }
        let m_conn_id = match msg {
            ConnectionMessage::Disconnected(id) => Some(id),
            ConnectionMessage::PacketReceived(id, _) => Some(id),
            _ => None
        };
        if let None = m_conn_id {
            return Err(anyhow!(format!("Hub attempted to handle an incoming message from a connection that it shouldn't: {:?}", msg)));
        }
        let conn_id = m_conn_id.unwrap();
        let state = self.get_conn(conn_id).await?.details.state;

        match state {
            ConnectionState::Entry => self.handle_connection_entry_message(msg).await,
            ConnectionState::LoggedIn => self.handle_connection_game_message(msg).await
        }
    }

    async fn handle_game_message(&self, msg: HubMessage) -> Result<()> {
        match msg {
            HubMessage::SendTo(id, pkt) => {
                self.send_msg(SendTo::Id(id), ConnectionMessage::SendPacket(pkt)).await?;
            }
            HubMessage::Broadcast(pkt) => {
                self.send_msg(SendTo::All, ConnectionMessage::SendPacket(pkt)).await?;
            }
            _ => {}
        };
        Ok(())
    }

    /// Listens for new connections and dispatches them
    async fn listen_loop(port: u16, connections: Arc<Mutex<HashMap<ConnId, ConnectionWrapper>>>, hub_tx: mpsc::UnboundedSender<ConnectionMessage>, broadcast_tx: broadcast::Sender<ConnectionMessage>, game_tx: mpsc::UnboundedSender<HubMessage>) -> Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let port = listener.local_addr()?.port();
        
        info!("Hub listening for connections on port {}", port);

        let mut conn_id = 0 as ConnId;

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
                lock.insert(conn_id, ConnectionWrapper { details: ConnectionDetails { id: conn_id, state: ConnectionState::Entry }, conn_tx });
            }

            // send connection message to game
            // _ = game_tx.send(HubMessage::PlayerJoined(conn_id));

            let conns = connections.clone();
            let hub_tx = hub_tx.clone();
            let broadcast_rx = broadcast_tx.subscribe();
            let conn = Connection::new(conn_id, conn_rx, hub_tx.clone(), broadcast_rx, stream);
            tokio::spawn(async move {
                conn.start().await?;
                
                // connection finished
                // send to hub so it can broadcast the message to others
                hub_tx.send(ConnectionMessage::Disconnected(conn_id))?;

                // cleanup
                let mut lock = conns.lock().await;
                lock.remove(&conn_id);
                debug!("Cleaned up after connection {}", conn_id);
                
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    async fn send_msg_to(&self, id: ConnId, msg: ConnectionMessage) -> Result<()> {
        let conn = self.get_conn(id).await?;
        conn.conn_tx.send(msg)?;
        Ok(())
    }

    async fn send_msg(&self, to: SendTo, msg: ConnectionMessage) -> Result<()> {
        match to {
            SendTo::Id(id) => self.send_msg_to(id, msg.clone()).await,
            SendTo::Matches(pred) => {
                // TODO: get parallel shit to work
                // as this is being processed in sequence

                let conns: Vec<_> = {
                    let lock = self.connections.lock().await;
                    lock.values().filter(|x| pred(x.details)).cloned().collect()
                };

                for conn in conns {
                    if pred(conn.details) {
                        self.send_msg_to(conn.details.id, msg.clone()).await?;
                    }
                }

                Ok(())
                
                // let mut set = JoinSet::new();
                // conns
                //     .iter()
                //     .map(|x| self.send_msg_to(x.details.id, msg.clone()))
                //     .for_each(|task| {
                //         set.spawn(task);
                //     });

                // set.join_all().await;
            },
            SendTo::All => {
                self.broadcast_tx.send(msg.clone())?;
                Ok(())
            }
        }
    }
}