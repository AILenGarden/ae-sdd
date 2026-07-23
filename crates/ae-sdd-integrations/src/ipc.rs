use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ae_sdd_protocol::{MAX_FRAME_BYTES, encode_frame};
use ae_sdd_runtime::ConnectionState;
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::tokio::{Listener, Stream, prelude::*};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use crate::{IntegrationError, IntegrationResult};

/// Bounded asynchronous per-user local IPC server.
pub struct LocalIpcServer {
    listener: Listener,
    active: Arc<AtomicUsize>,
    connections: Arc<Semaphore>,
    blocking_calls: Arc<Semaphore>,
    io_timeout: Duration,
}

impl LocalIpcServer {
    /// Binds a Windows Named Pipe or Unix UDS endpoint.
    pub fn bind(
        endpoint: &str,
        maximum_connections: usize,
        io_timeout: Duration,
    ) -> IntegrationResult<Self> {
        let name = endpoint_name(endpoint)?;
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(false)
            .create_tokio()?;
        let capacity = maximum_connections.max(1);
        Ok(Self {
            listener,
            active: Arc::new(AtomicUsize::new(0)),
            connections: Arc::new(Semaphore::new(capacity)),
            blocking_calls: Arc::new(Semaphore::new(capacity)),
            io_timeout,
        })
    }

    /// Accepts one connection and dispatches it as a bounded Tokio task.
    pub async fn accept_and_spawn<F>(&self, handler: Arc<F>) -> IntegrationResult<()>
    where
        F: Fn(&mut ConnectionState, &[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        let permit = Arc::clone(&self.connections)
            .acquire_owned()
            .await
            .map_err(|_| closed_server())?;
        let stream = self.listener.accept().await?;
        self.active.fetch_add(1, Ordering::AcqRel);
        let active = Arc::clone(&self.active);
        let blocking_calls = Arc::clone(&self.blocking_calls);
        let io_timeout = self.io_timeout;
        tokio::spawn(async move {
            let _guard = ActiveConnection {
                active,
                _permit: permit,
            };
            if let Err(error) = serve_connection(stream, io_timeout, blocking_calls, handler).await
            {
                tracing::debug!(error = %error, "local IPC connection closed with an error");
            }
        });
        Ok(())
    }

    /// Current connection handler count.
    #[must_use]
    pub fn active_connections(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    /// Waits until all accepted clients finish or the drain deadline expires.
    pub async fn wait_for_idle(&self, maximum: Duration) -> bool {
        let started = tokio::time::Instant::now();
        while self.active_connections() > 0 {
            if started.elapsed() >= maximum {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        true
    }
}

struct ActiveConnection {
    active: Arc<AtomicUsize>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for ActiveConnection {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

async fn serve_connection<F>(
    mut stream: Stream,
    io_timeout: Duration,
    blocking_calls: Arc<Semaphore>,
    handler: Arc<F>,
) -> IntegrationResult<()>
where
    F: Fn(&mut ConnectionState, &[u8]) -> Vec<u8> + Send + Sync + 'static,
{
    let mut connection = ConnectionState::default();
    loop {
        let mut prefix = [0_u8; 4];
        match timeout(io_timeout, stream.read_exact(&mut prefix)).await {
            Err(_) => return Err(timeout_error()),
            Ok(Ok(_)) => {}
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                return Ok(());
            }
            Ok(Err(error)) => return Err(error.into()),
        }
        let length = u32::from_be_bytes(prefix) as usize;
        if length == 0 || length > MAX_FRAME_BYTES {
            return Err(IntegrationError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid local IPC frame length",
            )));
        }
        let mut payload = vec![0_u8; length];
        timeout(io_timeout, stream.read_exact(&mut payload))
            .await
            .map_err(|_| timeout_error())??;
        let permit = Arc::clone(&blocking_calls)
            .acquire_owned()
            .await
            .map_err(|_| closed_server())?;
        let handler = Arc::clone(&handler);
        let (next_connection, response) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut connection = connection;
            let response = handler(&mut connection, &payload);
            (connection, response)
        })
        .await
        .map_err(|_| closed_server())?;
        connection = next_connection;
        let frame = encode_frame(&response).map_err(|_| {
            IntegrationError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "response exceeds frame limit",
            ))
        })?;
        timeout(io_timeout, stream.write_all(&frame))
            .await
            .map_err(|_| timeout_error())??;
        timeout(io_timeout, stream.flush())
            .await
            .map_err(|_| timeout_error())??;
    }
}

fn timeout_error() -> IntegrationError {
    IntegrationError::Io(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "local IPC deadline expired",
    ))
}

fn closed_server() -> IntegrationError {
    IntegrationError::Io(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "local IPC server is shutting down",
    ))
}

#[cfg(windows)]
fn endpoint_name(endpoint: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    endpoint.to_ns_name::<GenericNamespaced>()
}

#[cfg(unix)]
fn endpoint_name(endpoint: &str) -> std::io::Result<interprocess::local_socket::Name<'_>> {
    endpoint.to_fs_name::<GenericFilePath>()
}
