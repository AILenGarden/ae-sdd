//! Permissions shared by the client protocol and the service registry.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WriteLevel {
    #[default]
    None,
    Dml,
    Ddl,
}

impl WriteLevel {
    pub fn allows_dml(self) -> bool {
        matches!(self, Self::Dml | Self::Ddl)
    }

    pub fn allows_ddl(self) -> bool {
        matches!(self, Self::Ddl)
    }
}
