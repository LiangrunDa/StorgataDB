use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use bitcask_engine_rs::bitcask::{BitCask, KVStorage};
use crate::resp_codec::{convert_bulk_string_to_string, RespValue};
use crate::sync_layer::{RequestId, Syncable};

pub(crate) enum Cmd {
    /// Get the value of key. If the key does not exist the special value nil is returned.
    /// An error is returned if the value stored at key is not a string, because GET only handles string values.
    Get(RespValue),
    /// Set key to hold the `string` value. If key already holds a value, it is overwritten, regardless of its type.
    Set(RespValue, RespValue),
    Del(RespValue),
    Unknown,
}

impl Debug for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cmd::Get(key) => write!(f, "GET {:?}", key),
            Cmd::Set(key, value) => write!(f, "SET {:?} {:?}", key, value),
            Cmd::Del(key) => write!(f, "DEL {:?}", key),
            Cmd::Unknown => write!(f, "Unknown"),
        }
    }
}

impl From<RespValue> for Cmd {
    fn from(value: RespValue) -> Self {
        let cmd_map = |cmd: &str, arr: Vec<RespValue>| -> Cmd {
            match cmd {
                "GET" if arr.len() == 1 => Cmd::Get(arr[0].clone()),
                "SET" if arr.len() == 2 => Cmd::Set(arr[0].clone(), arr[1].clone()),
                "DEL" if arr.len() == 1 => Cmd::Del(arr[0].clone()),
                _ => Cmd::Unknown,
            }
        };

        match value {
            RespValue::Array(mut arr) if arr.iter().all(|v| matches!(v, RespValue::BulkString(_))) => {
                if let RespValue::BulkString(cmd_bytes) = arr.remove(0) {
                    let cmd = convert_bulk_string_to_string(cmd_bytes);
                    cmd_map(&cmd, arr)
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
    Put(RequestId, Vec<u8>, Vec<u8>),
    Del(RequestId, Vec<u8>),
}

impl Debug for InnerCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerCmd::Get(_, key) => write!(f, "GET {:?}", String::from_utf8_lossy(key)),
            InnerCmd::Put(_, key, value) => write!(f, "SET {:?} {:?}", String::from_utf8_lossy(key), String::from_utf8_lossy(value)),
            InnerCmd::Del(_, key) => write!(f, "DEL {:?}", String::from_utf8_lossy(key)),
        }
    }
}

impl Syncable for InnerCmd {
    fn handle(&self, storage: &mut BitCask) {
        match self {
            InnerCmd::Put(_, key, value) => {
                storage.put(key, value).unwrap_or_else(|_| ());
                println!("SET {:?} -> {:?}", key, value);
            }
            InnerCmd::Del(_, key) => {
                storage.delete(key).unwrap_or_else(|_| ());
                println!("DEL {:?}", key);
            }
            _ => {}
        }
    }

    fn get_request_id(&self) -> RequestId {
        match self {
            InnerCmd::Get(id, _) => *id,
            InnerCmd::Put(id, _, _) => *id,
            InnerCmd::Del(id, _) => *id,
        }
    }
}

impl InnerCmd {
    pub(crate) fn new(cmd: Cmd) -> anyhow::Result<Self> {
        let new_uuid = Uuid::new_v4();
        let id: RequestId = *new_uuid.as_bytes();

        match cmd {
            Cmd::Get(key) => {
                let key = convert_bulk_string_to_vec(key)?;
                Ok(Self::Get(id, key))
            }
            Cmd::Set(key, value) => {
                let key = convert_bulk_string_to_vec(key)?;
                let value = convert_bulk_string_to_vec(value)?;
                Ok(Self::Put(id, key, value))
            }
            Cmd::Del(key) => {
                let key = convert_bulk_string_to_vec(key)?;
                Ok(Self::Del(id, key))
            }
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