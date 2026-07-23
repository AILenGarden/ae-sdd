use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use ae_sdd_protocol::{MAX_FRAME_BYTES, encode_frame};
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::tokio::{Stream, prelude::*};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout as with_timeout;

use crate::{ClientError, ClientResult};

/// Injectable local request/response transport.
pub trait ClientTransport: Send + Sync {
    /// Exchanges ordered payloads on one authenticated connection.
    fn exchange<'a>(
        &'a self,
        endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = ClientResult<Vec<Vec<u8>>>> + Send + 'a>>;
}

/// Production Windows Named Pipe / Unix UDS transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalIpcTransport;

impl ClientTransport for LocalIpcTransport {
    fn exchange<'a>(
        &'a self,
        endpoint: &'a str,
        payloads: &'a [Vec<u8>],
        timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = ClientResult<Vec<Vec<u8>>>> + Send + 'a>> {
        Box::pin(async move {
            let name = endpoint_name(endpoint).map_err(|_| ClientError::DaemonUnavailable)?;
            let mut stream = with_timeout(timeout, Stream::connect(name))
                .await
                .map_err(|_| ClientError::DaemonUnavailable)?
                .map_err(|_| ClientError::DaemonUnavailable)?;
            let mut responses = Vec::with_capacity(payloads.len());
            for payload in payloads {
                let frame = encode_frame(payload).map_err(|_| ClientError::Protocol)?;
                with_timeout(timeout, stream.write_all(&frame))
                    .await
                    .map_err(|_| ClientError::DaemonUnavailable)?
                    .map_err(|_| ClientError::DaemonUnavailable)?;
                with_timeout(timeout, stream.flush())
                    .await
                    .map_err(|_| ClientError::DaemonUnavailable)?
                    .map_err(|_| ClientError::DaemonUnavailable)?;
                let mut prefix = [0_u8; 4];
                with_timeout(timeout, stream.read_exact(&mut prefix))
                    .await
                    .map_err(|_| ClientError::DaemonUnavailable)?
                    .map_err(|_| ClientError::DaemonUnavailable)?;
                let length = u32::from_be_bytes(prefix) as usize;
                if length == 0 || length > MAX_FRAME_BYTES {
                    return Err(ClientError::Protocol);
                }
                let mut response = vec![0_u8; length];
                with_timeout(timeout, stream.read_exact(&mut response))
                    .await
                    .map_err(|_| ClientError::DaemonUnavailable)?
                    .map_err(|_| ClientError::DaemonUnavailable)?;
                responses.push(response);
            }
            Ok(responses)
        })
    }
}

#[cfg(windows)]
fn endpoint_name(endpoint: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    endpoint.to_ns_name::<GenericNamespaced>()
}

#[cfg(unix)]
fn endpoint_name(endpoint: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    endpoint.to_fs_name::<GenericFilePath>()
}
