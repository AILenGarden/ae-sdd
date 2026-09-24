use std::path::Path;
use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::protocol::{
    MAX_FRAME_BYTES, ProtocolError, Request, Response, decode_response, encode_request,
};
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("daemon is unavailable: {0}")]
    Unavailable(String),
    #[error("IPC I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("IPC protocol failed: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("daemon response timed out")]
    Timeout,
    #[error("daemon closed the connection without a response")]
    EmptyResponse,
    #[error("IPC security policy is invalid: {0}")]
    Security(String),
}

pub const UNIX_SOCKET_MODE: u32 = 0o660;

impl IpcError {
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_) | Self::Timeout)
    }
}

pub fn endpoint_name() -> String {
    if let Ok(endpoint) = std::env::var("DB_OPERATOR_ENDPOINT")
        && !endpoint.trim().is_empty()
    {
        return endpoint;
    }
    endpoint_name_for("", Path::new(""))
}

pub fn endpoint_name_for(_user: &str, _config_path: &Path) -> String {
    if cfg!(windows) {
        r"\\.\pipe\db-operator-v2".to_owned()
    } else if cfg!(target_os = "macos") {
        "/var/run/db-operator/db-operator.sock".to_owned()
    } else {
        "/run/db-operator/db-operator.sock".to_owned()
    }
}

pub fn validate_pipe_sddl(sddl: &str) -> Result<(), IpcError> {
    let normalized = sddl.trim().to_ascii_uppercase();
    if !normalized.starts_with("D:P") {
        return Err(IpcError::Security(
            "pipe DACL must be protected with D:P".to_owned(),
        ));
    }
    for broad in [";;;WD)", ";;;AU)", ";;;BU)", ";;;AN)"] {
        if normalized.contains(broad) {
            return Err(IpcError::Security(
                "pipe DACL contains a broad principal".to_owned(),
            ));
        }
    }
    if !normalized.contains(";;;SY)") || !normalized.contains(";;;BA)") {
        return Err(IpcError::Security(
            "pipe DACL must include SYSTEM and Administrators".to_owned(),
        ));
    }
    if !normalized.contains("(A;;GRGW;;;S-1-5-") {
        return Err(IpcError::Security(
            "pipe DACL must include one explicit Agent SID with read/write access".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(all(windows, feature = "daemon"))]
pub fn create_secure_pipe_server(
    endpoint: &str,
    first_instance: bool,
    sddl: &str,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, IpcError> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use tokio::net::windows::named_pipe::ServerOptions;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};

    validate_pipe_sddl(sddl)?;
    let wide: Vec<u16> = std::ffi::OsStr::new(sddl)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(IpcError::Security(
            std::io::Error::last_os_error().to_string(),
        ));
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first_instance)
        .reject_remote_clients(true);
    let server = unsafe {
        options.create_with_security_attributes_raw(
            endpoint,
            (&mut attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    server.map_err(IpcError::Io)
}

pub async fn send_request(request: &Request, timeout: Duration) -> Result<Response, IpcError> {
    send_request_to(&endpoint_name(), request, timeout).await
}

pub async fn send_request_to(
    endpoint: &str,
    request: &Request,
    timeout: Duration,
) -> Result<Response, IpcError> {
    tokio::time::timeout(timeout, send_request_inner(endpoint, request))
        .await
        .map_err(|_| IpcError::Timeout)?
}

#[cfg(windows)]
async fn send_request_inner(endpoint: &str, request: &Request) -> Result<Response, IpcError> {
    use tokio::net::windows::named_pipe::ClientOptions;

    let stream = ClientOptions::new()
        .open(endpoint)
        .map_err(|error| IpcError::Unavailable(error.to_string()))?;
    exchange(stream, request).await
}

#[cfg(unix)]
async fn send_request_inner(endpoint: &str, request: &Request) -> Result<Response, IpcError> {
    let stream = tokio::net::UnixStream::connect(endpoint)
        .await
        .map_err(|error| IpcError::Unavailable(error.to_string()))?;
    exchange(stream, request).await
}

pub async fn exchange<S>(mut stream: S, request: &Request) -> Result<Response, IpcError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream.write_all(&encode_request(request)?).await?;
    stream.flush().await?;
    let frame = read_frame(stream).await?;
    decode_response(&frame).map_err(Into::into)
}

pub async fn read_frame<R>(reader: R) -> Result<Vec<u8>, IpcError>
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(reader);
    let mut frame = Vec::new();
    let bytes = reader.read_until(b'\n', &mut frame).await?;
    if bytes == 0 {
        return Err(IpcError::EmptyResponse);
    }
    if frame.len() > MAX_FRAME_BYTES + 1 {
        return Err(ProtocolError::FrameTooLarge.into());
    }
    Ok(frame)
}
