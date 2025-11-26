use std::{collections::HashMap, sync::Arc};

use anyhow::{Result, anyhow};
use log::{debug, error, info};
use tokio::{net::TcpListener, sync::{Mutex, mpsc}};

use crate::{messages::{ConnectionMessage, HubMessage}, net::connection::Connection, utils::TxRx};
use moonlapse_shared::{ConnId, packets::Packet};

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
    pub _id: ConnId,
    pub state: ConnectionState
}

#[derive(Clone, Debug)]
struct ConnectionWrapper {
    conn_tx: mpsc::Sender<ConnectionMessage>,
    details: ConnectionDetails
}

pub struct HubOptions {
    pub port: u16,
}

pub struct Hub {
    opts: HubOptions,
    hub_txrx: TxRx<ConnectionMessage>,
    game_txrx: TxRx<HubMessage>,
    connections: Arc<Mutex<HashMap<ConnId, ConnectionWrapper>>>
}

impl Hub {
    pub fn new(opts: HubOptions, game_txrx: TxRx<HubMessage>) -> Hub {
        let hub_txrx = mpsc::channel(1024);
        Hub { opts, hub_txrx, connections: Arc::new(Mutex::new(HashMap::new())), game_txrx }
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
        let hub_tx = self.hub_txrx.0.clone();
        tokio::spawn(async move {
            Self::listen_loop(port, connections, hub_tx).await?;
            Ok::<_, anyhow::Error>(())
        });

        // event reactive router
        // i.e. handle_msg
        loop {
            tokio::select! {
                Some(msg) = self.hub_txrx.1.recv() => {
                    match self.handle_connection_message(msg.clone()).await {
                        Err(e) => error!("Error handling incoming message {:?} from a connection: {}",  msg, e),
                        _ => {}
                    }
                }

                Some(msg) = self.game_txrx.1.recv() => {
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
                    let _ = self.game_txrx.0.send(HubMessage::PacketFromClient(id, p));
                }
            },
            ConnectionMessage::Disconnected(id) => {
                // forward to game to clean up. Nothing else required here since other cleanup is handled already.
                let _ = self.game_txrx.0.send(HubMessage::PlayerLeft(id));
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
                Packet::Register(_username, _password) => {

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
                            _ = self.game_txrx.0.send(HubMessage::PlayerJoined(id, username));
                        } else {
                            self.send_msg(SendTo::Id(id), ConnectionMessage::SendPacket(Packet::Deny("Incorrect password".into()))).await?;
                        }
                    } else {
                        self.send_msg(SendTo::Id(id), ConnectionMessage::SendPacket(Packet::Deny("User does not exist".into()))).await?;
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
    async fn listen_loop(port: u16, connections: Arc<Mutex<HashMap<ConnId, ConnectionWrapper>>>, hub_tx: mpsc::Sender<ConnectionMessage>) -> Result<()> {
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
            let (conn_tx, conn_rx) = mpsc::channel(1024);
            {
                let mut lock = connections.lock().await;
                lock.insert(conn_id, ConnectionWrapper { details: ConnectionDetails { _id: conn_id, state: ConnectionState::Entry }, conn_tx });
            }

            // send connection message to game
            // _ = game_tx.send(HubMessage::PlayerJoined(conn_id));

            let conns = connections.clone();
            let hub_tx = hub_tx.clone();
            let conn = Connection::new(conn_id, (hub_tx.clone(), conn_rx), stream);
            tokio::spawn(async move {
                conn.start().await?;
                
                // connection finished
                // send to hub so it can broadcast the message to others
                hub_tx.send(ConnectionMessage::Disconnected(conn_id)).await?;

                // cleanup
                let mut lock = conns.lock().await;
                lock.remove(&conn_id);
                debug!("Cleaned up after connection {}", conn_id);
                
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    async fn send_msg(&self, to: SendTo, msg: ConnectionMessage) -> Result<()> {
        // collect the senders we need to avoid holding the lock while sending/awaiting
        let targets: Vec<mpsc::Sender<ConnectionMessage>> = {
            let lock = self.connections.lock().await;
            
            match to {
                SendTo::Id(id) => {
                    if let Some(wrapper) = lock.get(&id) {
                        vec![wrapper.conn_tx.clone()]
                    } else {
                        vec![]
                    }
                },
                SendTo::All => {
                    lock.values().map(|c| c.conn_tx.clone()).collect()
                },
                SendTo::Matches(pred) => {
                    lock.values()
                        .filter(|c| pred(c.details))
                        .map(|c| c.conn_tx.clone())
                        .collect()
                }
            }
        };

        for tx in targets {
            // ignore errors (e.g. if client disconnected but map not updated yet)
            let _ = tx.send(msg.clone()); 
        }

        Ok(())
    }
}