use crate::cli::Args;
use crate::cmd::InnerCmd;
use crate::connection;
use crate::sync_layer::SyncRequest;
use bitcask_engine_rs::bitcask::BitCask;
use tokio::sync::mpsc;
use tracing::warn;

pub(crate) struct Server {
    args: Args,
    sync_request_tx: mpsc::Sender<SyncRequest<InnerCmd>>,
    storage: BitCask,
}

impl Server {
    pub(crate) fn new(
        args: Args,
        sync_request_tx: mpsc::Sender<SyncRequest<InnerCmd>>,
        storage: BitCask,
    ) -> Self {
        Self {
            args,
            sync_request_tx,
            storage,
        }
    }

    pub(crate) async fn run(&mut self) {
        let listener = tokio::net::TcpListener::bind(self.args.kv_addr())
            .await
            .unwrap();
        loop {
            let (socket, peer_addr) = listener.accept().await.unwrap();
            let mut connection = connection::Connection::new(
                socket,
                self.storage.clone(),
                self.sync_request_tx.clone(),
            );
            tokio::spawn(async move {
                connection.handle(peer_addr).await.unwrap_or_else(|e| {
                    warn!("Connection {} error: {}", peer_addr, e);
                });
            });
        }
    }
}
