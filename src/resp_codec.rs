use crate::connection::ConnectionError;
use async_recursion::async_recursion;
use std::fmt::Debug;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;

#[derive(Clone)]
pub(crate) enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Vec<RespValue>),
}

pub(crate) fn convert_bulk_string_to_string(bulk_string: Option<Vec<u8>>) -> String {
    match bulk_string {
        Some(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| String::new()),
        None => String::new(),
    }
}

impl Debug for RespValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RespValue::SimpleString(s) => write!(f, "SimpleString({})", s),
            RespValue::Error(e) => write!(f, "Error({})", e),
            RespValue::Integer(i) => write!(f, "Integer({})", i),
            RespValue::BulkString(bs) => {
                let bs = convert_bulk_string_to_string(bs.clone());
                write!(f, "BulkString({})", bs)
            }
            RespValue::Array(array) => write!(f, "Array({:?})", array),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RespCodec;

impl RespCodec {
    pub(crate) fn new() -> Self {
        Self {}
    }

    #[async_recursion]
    pub(crate) async fn decode<T: AsyncBufRead + Unpin + Send>(
        &mut self,
        input: &mut T,
    ) -> Result<RespValue, ConnectionError> {
        // Read the first byte to determine the type of the RESP value
        let mut buf = [0u8; 1];
        input.read_exact(&mut buf).await?;
        let value = match buf[0] {
            b'+' => {
                // Simple String
                let mut buf = Vec::new();
                input.read_until(b'\n', &mut buf).await?;
                let len = buf.len();
                if len < 2 || buf[len - 2] != b'\r' {
                    return Err(ConnectionError::IncompleteData);
                }
                RespValue::SimpleString(String::from_utf8(buf[..len - 2].to_vec())?)
            }
            b'-' => {
                // Error
                let mut buf = Vec::new();
                input.read_until(b'\n', &mut buf).await?;
                let len = buf.len();
                if len < 2 || buf[len - 2] != b'\r' {
                    return Err(ConnectionError::IncompleteData);
                }
                RespValue::Error(String::from_utf8(buf[..len - 2].to_vec())?)
            }
            b':' => {
                // Integer
                let mut buf = Vec::new();
                input.read_until(b'\n', &mut buf).await?;
                let len = buf.len();
                if len < 2 || buf[len - 2] != b'\r' {
                    return Err(ConnectionError::IncompleteData);
                }
                RespValue::Integer(String::from_utf8(buf[..len - 2].to_vec())?.parse::<i64>()?)
            }
            b'$' => {
                // Bulk String
                let mut buf = Vec::new();
                input.read_until(b'\n', &mut buf).await?;
                let len = buf.len();
                if len < 2 || buf[len - 2] != b'\r' {
                    return Err(ConnectionError::IncompleteData);
                }
                let len = String::from_utf8(buf[..len - 2].to_vec())?;
                let len = len.parse::<i32>()?;
                if len == -1 {
                    RespValue::BulkString(None)
                } else {
                    let mut buf = vec![0u8; len as usize + 2];
                    input.read_exact(&mut buf).await?;
                    if buf[len as usize] != b'\r' || buf[len as usize + 1] != b'\n' {
                        return Err(ConnectionError::IncompleteData);
                    }
                    RespValue::BulkString(Some(buf[..len as usize].to_vec()))
                }
            }
            b'*' => {
                // Array
                let mut buf = Vec::new();
                input.read_until(b'\n', &mut buf).await?;
                let len = buf.len();
                if len < 2 || buf[len - 2] != b'\r' {
                    return Err(ConnectionError::IncompleteData);
                }
                let len = String::from_utf8(buf[..len - 2].to_vec())?;
                let len = len.parse::<usize>()?;
                let mut array = Vec::with_capacity(len);
                for _ in 0..len {
                    array.push(self.decode(input).await?);
                }
                RespValue::Array(array)
            }
            _ => return Err(ConnectionError::UnrecognizedType),
        };
        match value.clone() {
            RespValue::BulkString(_) => {}
            _ => {
                debug!("Received {:?}", value);
            }
        }
        Ok(value)
    }

    #[async_recursion]
    pub(crate) async fn encode<T: AsyncWrite + Unpin + Send>(
        &mut self,
        output: &mut T,
        data: &RespValue,
    ) -> Result<(), ConnectionError> {
        debug!("Sending {:?}", data);
        match data {
            RespValue::SimpleString(s) => {
                output.write_all(b"+").await?;
                output.write_all(s.as_bytes()).await?;
                output.write_all(b"\r\n").await?;
            }
            RespValue::Error(e) => {
                output.write_all(b"-").await?;
                output.write_all(e.as_bytes()).await?;
                output.write_all(b"\r\n").await?;
            }
            RespValue::Integer(i) => {
                output.write_all(b":").await?;
                output.write_all(i.to_string().as_bytes()).await?;
                output.write_all(b"\r\n").await?;
            }
            RespValue::BulkString(bs) => {
                output.write_all(b"$").await?;
                if let Some(bs) = bs {
                    output.write_all(bs.len().to_string().as_bytes()).await?;
                    output.write_all(b"\r\n").await?;
                    output.write_all(bs).await?;
                    output.write_all(b"\r\n").await?;
                } else {
                    output.write_all(b"-1\r\n").await?;
                }
            }
            RespValue::Array(array) => {
                output.write_all(b"*").await?;
                output.write_all(array.len().to_string().as_bytes()).await?;
                output.write_all(b"\r\n").await?;
                for item in array {
                    self.encode(output, item).await?;
                }
            }
        }
        Ok(())
    }
}
