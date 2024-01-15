use crate::cli::Args;
use bitcask_engine_rs::bitcask::BitCask;
use raft_lite::config::{RaftConfig, RaftParams};
use raft_lite::persister::AsyncFilePersister;
use raft_lite::raft::Raft;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::warn;
use bitcask_engine_rs::error::BitCaskError;

pub(crate) type RequestId = [u8; 16];

pub(crate) trait Syncable: Serialize + DeserializeOwned + Send {
    fn handle(&self, storage: &mut BitCask) -> Result<(), BitCaskError>;
    fn get_request_id(&self) -> RequestId;
}

pub(crate) struct SyncRequest<M: Syncable> {
    pub(crate) message: M,
    pub(crate) answer: oneshot::Sender<Result<(), BitCaskError>>,
}

impl<M: Syncable> SyncRequest<M> {
    pub(crate) fn new(message: M, tx: oneshot::Sender<Result<(), BitCaskError>>) -> Self {
        Self {
            message,
            answer: tx,
        }
    }
}

pub(crate) struct SyncLayer {
    args: Args,
    storage: BitCask,
    request_map: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Result<(), BitCaskError>>>>>,
}

impl SyncLayer {
    pub(crate) fn new(args: Args, storage: BitCask) -> Self {
        let request_map = Arc::new(Mutex::new(HashMap::new()));
        Self {
            args,
            storage,
            request_map,
        }
    }

    pub(crate) async fn run<M: Syncable + 'static>(
        &mut self,
        mut sync_request_rx: mpsc::Receiver<SyncRequest<M>>,
    ) {
        let (mtx, mut mrx) = mpsc::channel::<Vec<u8>>(100);
        let (btx, brx) = mpsc::channel::<Vec<u8>>(100);
        let raft_config = RaftConfig::new(
            self.args.peer_addr(),
            self.args.self_addr(),
            RaftParams::default(),
            Box::new(AsyncFilePersister::new(self.args.raft_state_file())),
        );
        let mut raft = Raft::new(raft_config);
        raft.run(brx, mtx);

        // receive message from lower layer (Raft)
        let request_map = self.request_map.clone();
        let mut storage = self.storage.clone();
        tokio::spawn(async move {
            loop {
                let raw_payload = mrx.recv().await.unwrap();
                let sync_message: M = bincode::deserialize::<M>(&raw_payload).unwrap();
                let result = sync_message.handle(&mut storage);
                let request_id = sync_message.get_request_id();
                let mut request_map = request_map.lock().await;
                if let Some(tx) = request_map.remove(&request_id) {
                    if tx.send(result).is_err() {
                        warn!("SyncLayer: request_id {:?} is committed but the client is not aware of it", request_id);
                    }
                }
            }
        });

        // receive request from upper layer (application)
        let request_map = self.request_map.clone();
        tokio::spawn(async move {
            loop {
                let request = sync_request_rx
                    .recv()
                    .await
                    .expect("sync_request_rx closed");
                let raw_payload = bincode::serialize(&request.message).unwrap();
                let request_id = request.message.get_request_id();
                let mut request_map = request_map.lock().await;
                request_map.insert(request_id, request.answer);
                btx.send(raw_payload).await.unwrap();
            }
        });
    }
}
