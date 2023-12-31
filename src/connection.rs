use thiserror::Error;
use tokio::io::{ReadHalf, WriteHalf, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use bitcask_engine_rs::bitcask::{BitCask, KVStorage};
use crate::{cmd};
use crate::cmd::InnerCmd;
use crate::resp_codec::{RespCodec, RespValue};
use crate::sync_layer::SyncRequest;
use tokio::time::{timeout, Duration};
use tracing::{info};

#[derive(Error, Debug)]
pub(crate) enum ConnectionError {
    #[error("Incomplete data")]
    IncompleteData,
    #[error("Unrecognized type")]
    UnrecognizedType,
    #[error("IO error")]
    IoError(#[from] std::io::Error),
    #[error("Int unable to parse")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Not valid UTF8")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

pub(crate) struct Connection {
    reader: BufReader<ReadHalf<TcpStream>>,
    writer: WriteHalf<TcpStream>,
    codec: RespCodec,
    storage_handle: BitCask,
    sync_request_tx: mpsc::Sender<SyncRequest<InnerCmd>>,
}

// TODO: implement connection logger so that we can log the connection info (e.g. peer address)
impl Connection {
    pub(crate) fn new(stream: TcpStream, storage_handle: BitCask, sync_request_tx: mpsc::Sender<SyncRequest<InnerCmd>>) -> Self {
        let (reader, writer) = tokio::io::split(stream);
        let buf_reader = tokio::io::BufReader::new(reader);
        Self {
            reader: buf_reader,
            writer,
            storage_handle,
            codec: RespCodec::new(),
            sync_request_tx,
        }
    }

    pub(crate) async fn handle(&mut self) -> Result<(), ConnectionError>{
        loop {
            match self.codec.decode(&mut self.reader).await {
                Ok(res) => {
                    let cmd = cmd::Cmd::from(res.clone());
                    // the command could be well formatted but unknown
                    let parsed_inner_cmd = InnerCmd::new(cmd);
                    // if unknown command, here we will get an error
                    match parsed_inner_cmd {
                        Ok(inner_cmd) => {
                            self.handle_valid_cmd(inner_cmd).await?;
                        },
                        Err(_) => {
                            let msg = RespValue::Error(format!("Err unknown command {:?}", res));
                            // encode error must be IO error, so we can safely return here
                            self.codec.encode(&mut self.writer, &msg).await?;
                        }
                    }
                }
                // Could be EOF or other errors
                Err(e) => {
                    match e {
                        // If IO error is encountered, the connection should be closed
                        ConnectionError::IoError(e) => {
                            return if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                info!("Connection closed by client");
                                Ok(())
                            } else {
                                Err(e.into())
                            }
                        }
                        // Else, just log the error to client and continue since the command is not well formatted
                        _ => {
                            let msg = RespValue::Error(format!("Err {:?}", e));
                            self.codec.encode(&mut self.writer, &msg).await?;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn handle_valid_cmd(&mut self, inner_cmd: InnerCmd) -> Result<(), ConnectionError>{
        info!("Handling command: {:?}", inner_cmd);
        match inner_cmd {
            InnerCmd::Get(_, key) => {
                self.handle_read(key).await?;
            }
            InnerCmd::Put(_, _, _) | InnerCmd::Del(_, _) => {
                self.handle_write(inner_cmd).await?;
            }
        }
        Ok(())
    }

    /// Read the value from the storage and send it back to the client
    /// We don't need to synchronize the read operation with peers
    pub(crate) async fn handle_read(&mut self, key: Vec<u8>) -> Result<(), ConnectionError> {
        let value = self.storage_handle.get(&key);
        // value could be None, and it will be encoded as `$-1`
        let msg = RespValue::BulkString(value);
        // encode Error must be IO error, so we can safely return here
        self.codec.encode(&mut self.writer, &msg).await?;
        Ok(())
    }


    /// Write the value to the storage and send the response back to the client
    /// We need to synchronize the write operation with peers to guarantee consistency
    pub(crate) async fn handle_write(&mut self, inner_cmd: InnerCmd) -> Result<(), ConnectionError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sync_request = SyncRequest::new(inner_cmd, tx);
        self.sync_request_tx.send(sync_request).await.expect("Could not send sync request");
        // waiting for the response from the sync layer for 10 seconds
        match timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(_)) => {
                info!("Write operation is committed");
                let msg = RespValue::SimpleString("OK".to_string());
                self.codec.encode(&mut self.writer, &msg).await?;
            }
            Ok(Err(_)) => {
                let msg = RespValue::Error("Request timeout".to_string());
                self.codec.encode(&mut self.writer, &msg).await?;
            }
            Err(_) => {
                let msg = RespValue::Error("Internal error".to_string());
                self.codec.encode(&mut self.writer, &msg).await?;
            }
        }
        Ok(())
    }

}

