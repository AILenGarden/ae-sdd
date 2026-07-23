use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StringIdError {
    #[error("{kind} must not be empty")]
    Empty { kind: &'static str },
    #[error("{kind} exceeds its {max_bytes}-byte limit (actual: {actual_bytes})")]
    TooLong {
        kind: &'static str,
        max_bytes: usize,
        actual_bytes: usize,
    },
    #[error("{kind} must start with an ASCII alphanumeric character")]
    InvalidStart { kind: &'static str },
    #[error("{kind} contains invalid character {character:?} at byte {byte_index}")]
    InvalidCharacter {
        kind: &'static str,
        byte_index: usize,
        character: char,
    },
}

#[derive(Debug, Error)]
#[error("invalid {kind} UUID: {source}")]
pub struct UuidIdError {
    pub kind: &'static str,
    #[source]
    pub source: uuid::Error,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CounterError {
    #[error("{counter} cannot advance from {current} to {next}; the next value must be greater")]
    NotMonotonic {
        counter: &'static str,
        current: u64,
        next: u64,
    },
    #[error("{counter} overflowed at {current}")]
    Overflow { counter: &'static str, current: u64 },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DigestError {
    #[error("{kind} must contain exactly 64 lowercase hexadecimal characters")]
    InvalidEncoding { kind: &'static str },
}
