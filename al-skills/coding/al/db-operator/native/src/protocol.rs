use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::access::WriteLevel;

pub const PROTOCOL_VERSION: u32 = 2;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_SQL_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    pub request_id: String,
    pub capability: String,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Query {
        connection: String,
        sql: String,
        #[serde(default)]
        write_level: WriteLevel,
    },
    Status,
    SchemaGet {
        connection: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub version: u32,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetadata {
    pub mode: String,
    pub state: String,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("frame exceeds {MAX_FRAME_BYTES} bytes")]
    FrameTooLarge,
    #[error("invalid JSON request: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u32),
    #[error("request_id must not be empty")]
    MissingRequestId,
    #[error("capability must not be empty")]
    MissingCapability,
    #[error("SQL exceeds {MAX_SQL_BYTES} bytes")]
    SqlTooLarge,
}

pub fn encode_request(request: &Request) -> Result<Vec<u8>, ProtocolError> {
    validate_request(request)?;
    let mut encoded = serde_json::to_vec(request)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    encoded.push(b'\n');
    Ok(encoded)
}

pub fn encode_response(response: &Response) -> Result<Vec<u8>, ProtocolError> {
    let mut encoded = serde_json::to_vec(response)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    encoded.push(b'\n');
    Ok(encoded)
}

pub fn decode_request(frame: &[u8]) -> Result<Request, ProtocolError> {
    let frame = frame.strip_suffix(b"\n").unwrap_or(frame);
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let request: Request = serde_json::from_slice(frame)?;
    validate_request(&request)?;
    Ok(request)
}

fn validate_request(request: &Request) -> Result<(), ProtocolError> {
    if request.version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(request.version));
    }
    if request.request_id.trim().is_empty() {
        return Err(ProtocolError::MissingRequestId);
    }
    if request.capability.trim().is_empty() {
        return Err(ProtocolError::MissingCapability);
    }
    if let Operation::Query { sql, .. } = &request.operation
        && sql.len() > MAX_SQL_BYTES
    {
        return Err(ProtocolError::SqlTooLarge);
    }
    Ok(())
}

pub fn decode_response(frame: &[u8]) -> Result<Response, ProtocolError> {
    let frame = frame.strip_suffix(b"\n").unwrap_or(frame);
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    let response: Response = serde_json::from_slice(frame)?;
    if response.version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(response.version));
    }
    Ok(response)
}
