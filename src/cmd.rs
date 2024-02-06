use crate::resp_codec::{convert_bulk_string_to_string, RespValue};
use crate::sync_layer::{RequestId, Syncable};
use bitcask_engine_rs::bitcask::{BitCask, KVStorage, PutOption};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::info;
use uuid::Uuid;
use bitcask_engine_rs::error::BitCaskError;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct PutOptionSerde {
    pub(crate) nx: bool,
    pub(crate) xx: bool,
}

impl PutOptionSerde {
    pub fn nx() -> Option<Self> {
        Some(Self {
            nx: true,
            xx: false,
        })
    }

    pub fn xx() -> Option<Self> {
        Some(Self {
            nx: false,
            xx: true,
        })
    }
}

impl From<PutOptionSerde> for PutOption {
    fn from(put_option: PutOptionSerde) -> Self {
        Self {
            nx: put_option.nx,
            xx: put_option.xx,
        }
    }
}

pub(crate) enum Cmd {
    /// Get the value of key. If the key does not exist the special value nil is returned.
    /// An error is returned if the value stored at key is not a string, because GET only handles string values.
    Get(GetCmd),
    /// Set key to hold the `string` value. If key already holds a value, it is overwritten, regardless of its type.
    Set(SetCmd),
    Del(DelCmd),
    Ping,
    Unknown,
}

pub(crate) struct GetCmd {
    pub(crate) key: RespValue,
}

pub(crate) struct SetCmd {
    pub(crate) key: RespValue,
    pub(crate) value: RespValue,
    // could be NX or XX
    pub(crate) option: Option<PutOptionSerde>,
}

pub(crate) struct DelCmd {
    pub(crate) key: RespValue,
}

pub(crate) struct PingCmd;

impl Debug for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cmd::Get(cmd) => write!(f, "GET {:?}", cmd.key),
            Cmd::Set(cmd) => write!(f, "SET {:?} {:?}", cmd.key, cmd.value),
            Cmd::Del(cmd) => write!(f, "DEL {:?}", cmd.key),
            Cmd::Ping => write!(f, "PING"),
            Cmd::Unknown => write!(f, "Unknown"),
        }
    }
}

pub(crate) trait ParseCmd {
    fn parse(value: RespValue) -> anyhow::Result<Self>
        where
            Self: Sized;
}

impl ParseCmd for GetCmd {
    fn parse(value: RespValue) -> anyhow::Result<Self> {
        match value {
            RespValue::Array(mut arr) => {
                if arr.len() == 1 {
                    let key = arr.remove(0);
                    Ok(Self { key })
                } else {
                    Err(anyhow::anyhow!("Invalid GET command"))
                }
            }
            _ => Err(anyhow::anyhow!("Invalid GET command")),
        }
    }
}

impl ParseCmd for SetCmd {
    fn parse(value: RespValue) -> anyhow::Result<Self> {
        match value {
            RespValue::Array(mut arr) => {
                if arr.len() == 2 || arr.len() == 3 {
                    let key = arr.remove(0);
                    let value = arr.remove(0);
                    let option = if arr.len() == 1 {
                        let option = arr.remove(0);
                        match option {
                            RespValue::BulkString(bytes) => {
                                let option = convert_bulk_string_to_string(bytes);
                                match option.as_str() {
                                    "NX" => Ok(PutOptionSerde::nx()),
                                    "XX" => Ok(PutOptionSerde::xx()),
                                    _ => Err(anyhow::anyhow!("Invalid SET command")),
                                }
                            }
                            _ => Err(anyhow::anyhow!("Invalid SET command")),
                        }
                    } else {
                        Ok(None)
                    };
                    match option {
                        Ok(option) => Ok(Self { key, value, option }),
                        Err(e) => Err(e),
                    }
                } else {
                    Err(anyhow::anyhow!("Invalid SET command"))
                }
            }
            _ => Err(anyhow::anyhow!("Invalid SET command")),
        }
    }
}

impl ParseCmd for DelCmd {
    fn parse(value: RespValue) -> anyhow::Result<Self> {
        match value {
            RespValue::Array(mut arr) => {
                if arr.len() == 1 {
                    let key = arr.remove(0);
                    Ok(Self { key })
                } else {
                    Err(anyhow::anyhow!("Invalid DEL command"))
                }
            }
            _ => Err(anyhow::anyhow!("Invalid DEL command")),
        }
    }
}

impl ParseCmd for PingCmd {
    fn parse(value: RespValue) -> anyhow::Result<Self> {
        match value {
            RespValue::Array(arr) if arr.is_empty() => Ok(Self),
            _ => Err(anyhow::anyhow!("Invalid PING command")),
        }
    }
}

impl From<RespValue> for Cmd {
    fn from(value: RespValue) -> Self {
        match value {
            RespValue::Array(mut arr)
            if arr.iter().all(|v| matches!(v, RespValue::BulkString(_))) =>
                {
                    if let RespValue::BulkString(cmd_bytes) = arr.remove(0) {
                        let cmd = convert_bulk_string_to_string(cmd_bytes);
                        match cmd.as_str() {
                            "GET" => match GetCmd::parse(RespValue::Array(arr)) {
                                Ok(cmd) => Cmd::Get(cmd),
                                Err(_) => Cmd::Unknown,
                            },
                            "SET" => match SetCmd::parse(RespValue::Array(arr)) {
                                Ok(cmd) => Cmd::Set(cmd),
                                Err(_) => Cmd::Unknown,
                            },
                            "DEL" => match DelCmd::parse(RespValue::Array(arr)) {
                                Ok(cmd) => Cmd::Del(cmd),
                                Err(_) => Cmd::Unknown,
                            },
                            "PING" => match PingCmd::parse(RespValue::Array(arr)) {
                                Ok(_) => Cmd::Ping,
                                _ => Cmd::Unknown,
                            },
                            _ => Cmd::Unknown,
                        }
                    } else {
                        Cmd::Unknown
                    }
                }
            _ => Cmd::Unknown,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum InnerCmd {
    // String is request id
    Get(RequestId, Vec<u8>),
    // Key, Value, isNX
    Put(RequestId, Vec<u8>, Vec<u8>, Option<PutOptionSerde>),
    Del(RequestId, Vec<u8>),
    Ping,
}

impl Debug for InnerCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerCmd::Get(_, key) => write!(f, "GET {:?}", key),
            InnerCmd::Put(_, key, value, op) => {
                if let Some(op) = op {
                    write!(f, "SET {:?} {:?} with option {:?}", key, value, op)
                } else {
                    write!(f, "SET {:?} {:?}", key, value)
                }
            }
            InnerCmd::Del(_, key) => write!(f, "DEL {:?}", key),
            InnerCmd::Ping => write!(f, "PING"),
        }
    }
}

impl Syncable for InnerCmd {
    fn handle(&self, storage: &mut BitCask) -> Result<(), BitCaskError> {
        match self {
            InnerCmd::Put(_, key, value, option) => {
                let option = option.clone();
                let option = option.map(|op| op.into());
                storage.put_with_option(key, value, option)?;
                info!("SET {:?} -> {:?}", key, value);
                Ok(())
            }
            InnerCmd::Del(_, key) => {
                storage.delete(key)?;
                info!("DEL {:?}", key);
                Ok(())
            }
            _ => panic!("Command should not be handled by sync layer"),
        }
    }

    fn get_request_id(&self) -> RequestId {
        match self {
            InnerCmd::Get(id, _) => *id,
            InnerCmd::Put(id, _, _, _) => *id,
            InnerCmd::Del(id, _) => *id,
            InnerCmd::Ping => panic!("Ping command does not have request id"),
        }
    }
}

impl InnerCmd {
    pub(crate) fn new(cmd: Cmd) -> anyhow::Result<Self> {
        let new_uuid = Uuid::new_v4();
        let id: RequestId = *new_uuid.as_bytes();
        match cmd {
            Cmd::Get(cmd) => {
                let key = convert_bulk_string_to_vec(cmd.key)?;
                Ok(Self::Get(id, key))
            }
            Cmd::Set(cmd) => {
                let key = convert_bulk_string_to_vec(cmd.key)?;
                let value = convert_bulk_string_to_vec(cmd.value)?;
                let option = cmd.option;
                Ok(Self::Put(id, key, value, option))
            }
            Cmd::Del(cmd) => {
                let key = convert_bulk_string_to_vec(cmd.key)?;
                Ok(Self::Del(id, key))
            }
            Cmd::Ping => Ok(Self::Ping),
            Cmd::Unknown => Err(anyhow::anyhow!("Unknown command")),
        }
    }
}

fn convert_bulk_string_to_vec(bulk_string: RespValue) -> anyhow::Result<Vec<u8>> {
    match bulk_string {
        RespValue::BulkString(Some(bytes)) => Ok(bytes),
        RespValue::BulkString(None) => Err(anyhow::anyhow!("None bulk string")),
        _ => Err(anyhow::anyhow!("Invalid bulk string")),
    }
}

