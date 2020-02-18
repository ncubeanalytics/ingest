use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use actix::Addr;
use futures::future::{join_all, FutureExt};
use tokio::sync::RwLock;
use tracing::{error, trace};

use crate::kafka::Kafka;

use super::connection::ws::{WSClose, WSHandler};

const DEFAULT_WS_CAP: usize = 128;

pub struct ServerState {
    pub kafka: Kafka,
    ws_connections: RwLock<HashSet<Addr<WSHandler>>>,
    accept_ws: AtomicBool,
}

impl ServerState {
    pub fn new(kafka: Kafka) -> Self {
        Self {
            kafka,
            ws_connections: RwLock::new(HashSet::with_capacity(DEFAULT_WS_CAP)),
            accept_ws: AtomicBool::new(true),
        }
    }

    pub async fn register_ws(&self, ws_addr: Addr<WSHandler>) {
        let mut ws_connections = self.ws_connections.write().await;

        if !ws_connections.insert(ws_addr) {
            error!("Tried to register same webscket actor twice");
        } else {
            trace!("Registered new websocket connection")
        }
    }

    pub async fn unregister_ws(&self, ws_addr: &Addr<WSHandler>) {
        let mut ws_connections = self.ws_connections.write().await;

        if !ws_connections.remove(ws_addr) {
            error!("Tried to unregister nonexistent websocket actor");
        } else {
            trace!("Unregistered websocket connection");
        }
    }

    pub async fn close_all_ws(&self) {
        self.accept_ws.store(false, Relaxed);

        let ws_connections = self.ws_connections.read().await;

        join_all(ws_connections.iter().map(|ws_conn| {
            ws_conn.send(WSClose).map(|r| {
                if let Err(e) = r {
                    error!(
                        "Could not send internal close message to websocket connection: {}",
                        e
                    );
                }
            })
        }))
        .await;
    }

    pub fn accepting_ws(&self) -> bool {
        self.accept_ws.load(Relaxed)
    }
}
