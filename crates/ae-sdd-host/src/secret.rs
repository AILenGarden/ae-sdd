use std::fmt;

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

pub struct EndpointSecret([u8; 32]);

impl EndpointSecret {
    #[must_use]
    pub fn generate() -> Self {
        let mut value = [0_u8; 32];
        OsRng.fill_bytes(&mut value);
        Self(value)
    }

    #[must_use]
    pub fn expose_hex(&self) -> String {
        hex::encode(self.0)
    }

    #[must_use]
    pub fn digest_hex(&self) -> String {
        hex::encode(Sha256::digest(self.0))
    }

    #[must_use]
    pub fn authenticate_hex(&self, candidate: &str) -> bool {
        let mut decoded = [0_u8; 32];
        if hex::decode_to_slice(candidate, &mut decoded).is_err() {
            return false;
        }
        self.0
            .iter()
            .zip(decoded)
            .fold(0_u8, |difference, (expected, actual)| {
                difference | (expected ^ actual)
            })
            == 0
    }
}

impl fmt::Debug for EndpointSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EndpointSecret([REDACTED])")
    }
}

impl Drop for EndpointSecret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_secret_authenticates_but_cannot_expose_debug_bytes() {
        let secret = EndpointSecret::generate();
        let encoded = secret.expose_hex();

        assert!(secret.authenticate_hex(&encoded));
        assert!(!secret.authenticate_hex(&"00".repeat(32)));
        assert!(!secret.authenticate_hex("not-hex"));
        assert_eq!(format!("{secret:?}"), "EndpointSecret([REDACTED])");
        assert_ne!(secret.digest_hex(), encoded);
    }
}
