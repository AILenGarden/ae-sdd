use std::fmt;

use serde::{Deserialize, Serialize};

/// A wire secret whose `Debug` representation never exposes its contents.
///
/// Serialization intentionally emits the original value because endpoint
/// authentication requires it on the local wire. Logs must use `Debug`, not
/// [`SecretString::expose_secret`].
#[derive(Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps a secret that must be transmitted over the authenticated local endpoint.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Exposes the secret for constant-time authentication at the daemon boundary.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}
