//! One-shot child bootstrap channel.
//!
//! The bridge holds a delegation claim in memory and hands it to exactly one
//! physical child process over a randomly named, current-user-only local
//! endpoint. The raw claim never enters argv, environment variables, files,
//! logs, or any persisted record; the child receives only three bounded
//! correlation values and proves possession of them as a challenge.

use std::fmt;
use std::time::Duration;

#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::tokio::{Listener, Stream, prelude::*};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Instant, timeout};
use uuid::Uuid;

/// Environment variable naming the one-shot bootstrap endpoint.
pub const ENV_BOOTSTRAP_ENDPOINT: &str = "AE_SDD_CHILD_BOOTSTRAP_ENDPOINT";
/// Environment variable naming the daemon-minted child session identity.
pub const ENV_CHILD_SESSION_ID: &str = "AE_SDD_CHILD_SESSION_ID";
/// Environment variable naming the owning host action.
pub const ENV_HOST_ACTION_ID: &str = "AE_SDD_HOST_ACTION_ID";

/// Largest accepted challenge or envelope frame.
const MAX_BOOTSTRAP_FRAME_BYTES: usize = 8 * 1024;
/// Deadline for a single read or write on the bootstrap endpoint.
const BOOTSTRAP_IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Correlation values a child must present to claim its bootstrap envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChildBootstrapChallenge {
    /// Daemon-minted child session identity.
    pub child_session_id: Uuid,
    /// Host action that authorised the spawn.
    pub host_action_id: Uuid,
}

/// Bootstrap facts delivered to a verified child process.
#[derive(Clone)]
pub struct ChildBootstrapEnvelope {
    /// Daemon-minted child session identity.
    pub child_session_id: Uuid,
    /// Host action that authorised the spawn.
    pub host_action_id: Uuid,
    /// Single-use delegation claim.
    pub claim: String,
}

impl fmt::Debug for ChildBootstrapEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChildBootstrapEnvelope")
            .field("child_session_id", &self.child_session_id)
            .field("host_action_id", &self.host_action_id)
            .field("claim", &"<redacted>")
            .finish()
    }
}

/// One-shot bootstrap channel failure.
#[derive(Debug, Error)]
pub enum ChildBootstrapError {
    /// The endpoint could not be created, connected to, or accepted on.
    #[error("one-shot bootstrap endpoint failed")]
    Endpoint,
    /// The presented challenge did not match the published one.
    #[error("child bootstrap challenge did not match")]
    ChallengeMismatch,
    /// The channel deadline passed before a verified consumer arrived.
    #[error("child bootstrap channel expired")]
    Expired,
    /// A frame exceeded the bootstrap budget or was malformed.
    #[error("child bootstrap frame was invalid")]
    InvalidFrame,
}

impl From<std::io::Error> for ChildBootstrapError {
    fn from(_: std::io::Error) -> Self {
        Self::Endpoint
    }
}

/// Challenge wire frame. Identities travel as canonical strings because the
/// workspace `uuid` dependency intentionally excludes the `serde` feature.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChallengeFrame {
    child_session_id: String,
    host_action_id: String,
}

/// Envelope wire frame.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeFrame {
    child_session_id: String,
    host_action_id: String,
    claim: String,
}

/// A published, single-consumer bootstrap endpoint holding one claim.
pub struct OneShotBootstrapChannel {
    endpoint: String,
    challenge: ChildBootstrapChallenge,
    /// Held as raw bytes so the buffer can be overwritten without `unsafe`.
    claim: Vec<u8>,
    listener: Option<Listener>,
    deadline: Instant,
}

impl fmt::Debug for OneShotBootstrapChannel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OneShotBootstrapChannel")
            .field("endpoint", &self.endpoint)
            .field("challenge", &self.challenge)
            .field("claim", &"<redacted>")
            .finish()
    }
}

impl OneShotBootstrapChannel {
    /// Publishes a randomly named current-user-only endpoint holding one claim.
    pub fn publish(
        challenge: ChildBootstrapChallenge,
        claim: String,
        lifetime: Duration,
    ) -> Result<Self, ChildBootstrapError> {
        let endpoint = format!("ae-sdd-child-bootstrap-{}", Uuid::new_v4().simple());
        let listener = ListenerOptions::new()
            .name(endpoint_name(&endpoint)?)
            .try_overwrite(false)
            .create_tokio()?;
        Ok(Self {
            endpoint,
            challenge,
            claim: claim.into_bytes(),
            listener: Some(listener),
            deadline: Instant::now() + lifetime,
        })
    }

    /// Endpoint name handed to the child through its environment.
    #[must_use]
    pub fn endpoint_name(&self) -> &str {
        &self.endpoint
    }

    /// The three bounded correlation variables exported to the child process.
    ///
    /// This set is the complete allowlist; the raw claim is never included.
    #[must_use]
    pub fn child_environment(&self) -> Vec<(String, String)> {
        vec![
            (ENV_BOOTSTRAP_ENDPOINT.to_owned(), self.endpoint.clone()),
            (
                ENV_CHILD_SESSION_ID.to_owned(),
                self.challenge.child_session_id.to_string(),
            ),
            (
                ENV_HOST_ACTION_ID.to_owned(),
                self.challenge.host_action_id.to_string(),
            ),
        ]
    }

    /// Serves exactly one consumer, then closes the endpoint and drops the claim.
    ///
    /// Taking `self.listener` is what makes the channel single-use: once this
    /// returns, the endpoint is unbound regardless of outcome.
    pub async fn serve_once(mut self) -> Result<(), ChildBootstrapError> {
        let listener = self.listener.take().ok_or(ChildBootstrapError::Endpoint)?;
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or(ChildBootstrapError::Expired)?;

        let mut stream = timeout(remaining, listener.accept())
            .await
            .map_err(|_| ChildBootstrapError::Expired)??;
        drop(listener);

        let presented: ChallengeFrame = read_frame(&mut stream).await?;
        if presented.child_session_id != self.challenge.child_session_id.to_string()
            || presented.host_action_id != self.challenge.host_action_id.to_string()
        {
            return Err(ChildBootstrapError::ChallengeMismatch);
        }

        let claim =
            String::from_utf8(self.claim.clone()).map_err(|_| ChildBootstrapError::InvalidFrame)?;
        let frame = EnvelopeFrame {
            child_session_id: self.challenge.child_session_id.to_string(),
            host_action_id: self.challenge.host_action_id.to_string(),
            claim,
        };
        write_frame(&mut stream, &frame).await
    }

    /// Connects as the child and claims the bootstrap envelope.
    pub async fn consume(
        endpoint: &str,
        challenge: ChildBootstrapChallenge,
    ) -> Result<ChildBootstrapEnvelope, ChildBootstrapError> {
        let mut stream = Stream::connect(endpoint_name(endpoint)?).await?;
        let request = ChallengeFrame {
            child_session_id: challenge.child_session_id.to_string(),
            host_action_id: challenge.host_action_id.to_string(),
        };
        write_frame(&mut stream, &request).await?;
        let frame: EnvelopeFrame = read_frame(&mut stream).await?;
        let child_session_id = Uuid::parse_str(&frame.child_session_id)
            .map_err(|_| ChildBootstrapError::InvalidFrame)?;
        let host_action_id = Uuid::parse_str(&frame.host_action_id)
            .map_err(|_| ChildBootstrapError::InvalidFrame)?;
        if child_session_id != challenge.child_session_id
            || host_action_id != challenge.host_action_id
        {
            return Err(ChildBootstrapError::ChallengeMismatch);
        }
        Ok(ChildBootstrapEnvelope {
            child_session_id,
            host_action_id,
            claim: frame.claim,
        })
    }
}

impl Drop for OneShotBootstrapChannel {
    fn drop(&mut self) {
        // Overwrite the claim bytes before the allocation is released.
        self.claim.fill(0);
        self.claim.clear();
        self.listener = None;
    }
}

async fn read_frame<T: for<'de> Deserialize<'de>>(
    stream: &mut Stream,
) -> Result<T, ChildBootstrapError> {
    let mut prefix = [0_u8; 4];
    timeout(BOOTSTRAP_IO_TIMEOUT, stream.read_exact(&mut prefix))
        .await
        .map_err(|_| ChildBootstrapError::Expired)??;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAX_BOOTSTRAP_FRAME_BYTES {
        return Err(ChildBootstrapError::InvalidFrame);
    }
    let mut payload = vec![0_u8; length];
    timeout(BOOTSTRAP_IO_TIMEOUT, stream.read_exact(&mut payload))
        .await
        .map_err(|_| ChildBootstrapError::Expired)??;
    serde_json::from_slice(&payload).map_err(|_| ChildBootstrapError::InvalidFrame)
}

async fn write_frame<T: Serialize>(
    stream: &mut Stream,
    value: &T,
) -> Result<(), ChildBootstrapError> {
    let payload = serde_json::to_vec(value).map_err(|_| ChildBootstrapError::InvalidFrame)?;
    if payload.is_empty() || payload.len() > MAX_BOOTSTRAP_FRAME_BYTES {
        return Err(ChildBootstrapError::InvalidFrame);
    }
    let length = u32::try_from(payload.len()).map_err(|_| ChildBootstrapError::InvalidFrame)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    timeout(BOOTSTRAP_IO_TIMEOUT, stream.write_all(&frame))
        .await
        .map_err(|_| ChildBootstrapError::Expired)??;
    timeout(BOOTSTRAP_IO_TIMEOUT, stream.flush())
        .await
        .map_err(|_| ChildBootstrapError::Expired)??;
    Ok(())
}

#[cfg(windows)]
fn endpoint_name(
    endpoint: &str,
) -> Result<interprocess::local_socket::Name<'_>, ChildBootstrapError> {
    endpoint
        .to_ns_name::<GenericNamespaced>()
        .map_err(|_| ChildBootstrapError::Endpoint)
}

#[cfg(unix)]
fn endpoint_name(
    endpoint: &str,
) -> Result<interprocess::local_socket::Name<'_>, ChildBootstrapError> {
    endpoint
        .to_fs_name::<GenericFilePath>()
        .map_err(|_| ChildBootstrapError::Endpoint)
}
