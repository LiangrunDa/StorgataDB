use crate::cmd::InnerCmd;
use crate::logger::init_logger;
use crate::sync_layer::SyncLayer;
use tracing::{debug, info};

mod cli;
mod cmd;
mod connection;
mod logger;
mod resp_codec;
mod server;
mod sync_layer;

fn main() {
    let args = cli::parse_args();
    init_logger(
        args.log_level(),
        args.log_dir(),
        args.log_file(),
        args.rust_log(),
    )
    .expect("Could not initialize logger");
    info!("Starting with args: {:?}", args);
    debug!("Starting debug");
    let storage = bitcask_engine_rs::bitcask::BitCask::new(args.data_dir()).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let (sync_request_tx, sync_request_rx) =
            tokio::sync::mpsc::channel::<sync_layer::SyncRequest<InnerCmd>>(100);
        let mut sync_layer = SyncLayer::new(args.clone(), storage.clone());
        let sync_layer_task = sync_layer.run(sync_request_rx);
        let mut server = server::Server::new(args, sync_request_tx, storage);
        let server_task = server.run();
        tokio::join!(sync_layer_task, server_task)
    });
}
