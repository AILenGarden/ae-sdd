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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use interprocess::local_socket::ListenerOptions;
    use interprocess::local_socket::tokio::Listener;

    use super::*;

    static ENDPOINT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    /// Unique per test so concurrent runs never collide on one pipe name.
    fn unique_endpoint() -> String {
        let sequence = ENDPOINT_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        format!("ae-sdd-transport-test-{}-{sequence}", std::process::id())
    }

    #[cfg(windows)]
    fn listener_for(endpoint: &str) -> Listener {
        ListenerOptions::new()
            .name(
                endpoint
                    .to_ns_name::<GenericNamespaced>()
                    .expect("valid ns name"),
            )
            .create_tokio()
            .expect("listener binds")
    }

    #[cfg(unix)]
    fn listener_for(endpoint: &str) -> Listener {
        let path = std::env::temp_dir().join(endpoint);
        let _ = std::fs::remove_file(&path);
        ListenerOptions::new()
            .name(
                path.to_str()
                    .expect("utf-8 path")
                    .to_fs_name::<GenericFilePath>()
                    .expect("valid fs name"),
            )
            .create_tokio()
            .expect("listener binds")
    }

    #[cfg(windows)]
    fn endpoint_arg(endpoint: &str) -> String {
        endpoint.to_owned()
    }

    #[cfg(unix)]
    fn endpoint_arg(endpoint: &str) -> String {
        std::env::temp_dir()
            .join(endpoint)
            .to_str()
            .expect("utf-8 path")
            .to_owned()
    }

    #[tokio::test]
    async fn connecting_to_an_absent_daemon_is_unavailable_not_a_protocol_error() {
        // Nothing is listening on this name, so the classification must be
        // `DaemonUnavailable` — that is what makes `call_with_ensure` retry.
        let endpoint = endpoint_arg(&unique_endpoint());
        let error = LocalIpcTransport
            .exchange(
                &endpoint,
                &[b"payload".to_vec()],
                Duration::from_millis(250),
            )
            .await
            .expect_err("no listener means no exchange");

        assert!(
            matches!(error, ClientError::DaemonUnavailable),
            "expected DaemonUnavailable, got {error:?}"
        );
    }

    #[tokio::test]
    async fn a_zero_length_response_frame_is_a_protocol_violation() {
        // A reachable peer that answers with a 0-length prefix is speaking the
        // framing wrong; that must not be mistaken for unavailability.
        let endpoint = unique_endpoint();
        let listener = listener_for(&endpoint);
        let server = tokio::spawn(async move {
            let stream = listener.accept().await.expect("accept");
            let (mut reader, mut writer) = tokio::io::split(stream);
            let mut prefix = [0_u8; 4];
            reader.read_exact(&mut prefix).await.expect("length prefix");
            let length = u32::from_be_bytes(prefix) as usize;
            let mut body = vec![0_u8; length];
            reader.read_exact(&mut body).await.expect("request body");
            writer
                .write_all(&0_u32.to_be_bytes())
                .await
                .expect("zero-length reply");
            writer.flush().await.expect("flush");
        });

        let error = LocalIpcTransport
            .exchange(
                &endpoint_arg(&endpoint),
                &[b"request".to_vec()],
                Duration::from_secs(5),
            )
            .await
            .expect_err("a zero-length frame must be rejected");

        assert!(
            matches!(error, ClientError::Protocol),
            "expected Protocol, got {error:?}"
        );
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn an_oversized_response_frame_is_rejected_before_allocating_it() {
        // The length guard exists so a hostile/buggy peer cannot make the
        // client allocate an unbounded buffer.
        let endpoint = unique_endpoint();
        let listener = listener_for(&endpoint);
        let server = tokio::spawn(async move {
            let stream = listener.accept().await.expect("accept");
            let (mut reader, mut writer) = tokio::io::split(stream);
            let mut prefix = [0_u8; 4];
            reader.read_exact(&mut prefix).await.expect("length prefix");
            let length = u32::from_be_bytes(prefix) as usize;
            let mut body = vec![0_u8; length];
            reader.read_exact(&mut body).await.expect("request body");
            let oversized = u32::try_from(MAX_FRAME_BYTES + 1).expect("fits u32");
            writer
                .write_all(&oversized.to_be_bytes())
                .await
                .expect("oversized prefix");
            writer.flush().await.expect("flush");
        });

        let error = LocalIpcTransport
            .exchange(
                &endpoint_arg(&endpoint),
                &[b"request".to_vec()],
                Duration::from_secs(5),
            )
            .await
            .expect_err("an oversized frame must be rejected");

        assert!(
            matches!(error, ClientError::Protocol),
            "expected Protocol, got {error:?}"
        );
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn a_well_framed_reply_round_trips_for_every_payload_in_order() {
        // Two payloads on one connection: proves the loop reuses the stream and
        // preserves response ordering.
        let endpoint = unique_endpoint();
        let listener = listener_for(&endpoint);
        let server = tokio::spawn(async move {
            let stream = listener.accept().await.expect("accept");
            let (mut reader, mut writer) = tokio::io::split(stream);
            for reply in [b"first".as_slice(), b"second".as_slice()] {
                let mut prefix = [0_u8; 4];
                reader.read_exact(&mut prefix).await.expect("length prefix");
                let length = u32::from_be_bytes(prefix) as usize;
                let mut body = vec![0_u8; length];
                reader.read_exact(&mut body).await.expect("request body");
                let out = u32::try_from(reply.len()).expect("fits u32");
                writer.write_all(&out.to_be_bytes()).await.expect("prefix");
                writer.write_all(reply).await.expect("body");
                writer.flush().await.expect("flush");
            }
        });

        let responses = LocalIpcTransport
            .exchange(
                &endpoint_arg(&endpoint),
                &[b"one".to_vec(), b"two".to_vec()],
                Duration::from_secs(5),
            )
            .await
            .expect("both payloads exchange");

        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0], b"first".to_vec());
        assert_eq!(responses[1], b"second".to_vec());
        server.await.expect("server task");
    }
}
