use std::collections::BTreeMap;

use ae_sdd_domain::InputFingerprint;
use sha2::{Digest, Sha256};
use thiserror::Error;
use yaml_rust2::{Yaml, YamlLoader};

pub const DEFAULT_MAX_YAML_BYTES: usize = 256 * 1024;
const MAX_YAML_DEPTH: usize = 64;
const MAX_YAML_NODES: usize = 10_000;

/// Restricted YAML 1.2 value tree accepted by ae-sdd configuration readers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum YamlValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(Box<str>),
    String(Box<str>),
    Sequence(Vec<YamlValue>),
    Mapping(BTreeMap<Box<str>, YamlValue>),
}

impl YamlValue {
    pub fn mapping(&self) -> Option<&BTreeMap<Box<str>, YamlValue>> {
        match self {
            Self::Mapping(value) => Some(value),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Real(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YamlDocument {
    root: YamlValue,
}

impl YamlDocument {
    pub fn parse(input: &[u8]) -> Result<Self, YamlError> {
        Self::parse_bounded(input, DEFAULT_MAX_YAML_BYTES)
    }

    pub fn parse_bounded(input: &[u8], max_bytes: usize) -> Result<Self, YamlError> {
        if input.len() > max_bytes {
            return Err(YamlError::ByteLimit {
                actual: input.len(),
                maximum: max_bytes,
            });
        }
        let text = std::str::from_utf8(input).map_err(|_| YamlError::NotUtf8)?;
        let docs = YamlLoader::load_from_str(text).map_err(|error| YamlError::Parse {
            message: error.to_string(),
        })?;
        if docs.len() != 1 {
            return Err(YamlError::DocumentCount { actual: docs.len() });
        }
        let mut budget = NodeBudget { nodes: 0 };
        Ok(Self {
            root: convert(&docs[0], 0, &mut budget)?,
        })
    }

    pub const fn root(&self) -> &YamlValue {
        &self.root
    }

    pub fn get(&self, key: &str) -> Option<&YamlValue> {
        self.root.mapping()?.get(key)
    }

    pub fn fingerprint(&self) -> InputFingerprint {
        let mut hasher = Sha256::new();
        hasher.update(b"ae-sdd-yaml/v1\0");
        hash_value(&mut hasher, &self.root);
        InputFingerprint::from_array(hasher.finalize().into())
    }
}

struct NodeBudget {
    nodes: usize,
}

fn convert(value: &Yaml, depth: usize, budget: &mut NodeBudget) -> Result<YamlValue, YamlError> {
    if depth > MAX_YAML_DEPTH {
        return Err(YamlError::DepthLimit {
            maximum: MAX_YAML_DEPTH,
        });
    }
    budget.nodes += 1;
    if budget.nodes > MAX_YAML_NODES {
        return Err(YamlError::NodeLimit {
            maximum: MAX_YAML_NODES,
        });
    }
    match value {
        Yaml::Null => Ok(YamlValue::Null),
        Yaml::Boolean(value) => Ok(YamlValue::Boolean(*value)),
        Yaml::Integer(value) => Ok(YamlValue::Integer(*value)),
        Yaml::Real(value) => Ok(YamlValue::Real(value.clone().into())),
        Yaml::String(value) => Ok(YamlValue::String(value.clone().into())),
        Yaml::Array(values) => values
            .iter()
            .map(|value| convert(value, depth + 1, budget))
            .collect::<Result<_, _>>()
            .map(YamlValue::Sequence),
        Yaml::Hash(values) => {
            let mut mapping = BTreeMap::new();
            for (key, value) in values {
                let Yaml::String(key) = key else {
                    return Err(YamlError::NonStringMappingKey);
                };
                if mapping
                    .insert(key.clone().into(), convert(value, depth + 1, budget)?)
                    .is_some()
                {
                    return Err(YamlError::DuplicateMappingKey(key.clone()));
                }
            }
            Ok(YamlValue::Mapping(mapping))
        }
        Yaml::Alias(_) => Err(YamlError::AliasUnsupported),
        Yaml::BadValue => Err(YamlError::BadValue),
    }
}

fn hash_value(hasher: &mut Sha256, value: &YamlValue) {
    match value {
        YamlValue::Null => hasher.update([0]),
        YamlValue::Boolean(value) => hasher.update([1, u8::from(*value)]),
        YamlValue::Integer(value) => {
            hasher.update([2]);
            hasher.update(value.to_be_bytes());
        }
        YamlValue::Real(value) => hash_scalar(hasher, 3, value.as_bytes()),
        YamlValue::String(value) => hash_scalar(hasher, 4, value.as_bytes()),
        YamlValue::Sequence(values) => {
            hasher.update([5]);
            hasher.update((values.len() as u64).to_be_bytes());
            for value in values {
                hash_value(hasher, value);
            }
        }
        YamlValue::Mapping(values) => {
            hasher.update([6]);
            hasher.update((values.len() as u64).to_be_bytes());
            for (key, value) in values {
                hash_scalar(hasher, 7, key.as_bytes());
                hash_value(hasher, value);
            }
        }
    }
}

fn hash_scalar(hasher: &mut Sha256, tag: u8, bytes: &[u8]) {
    hasher.update([tag]);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum YamlError {
    #[error("YAML input has {actual} bytes, exceeding {maximum}")]
    ByteLimit { actual: usize, maximum: usize },
    #[error("YAML input is not UTF-8")]
    NotUtf8,
    #[error("YAML parse failed: {message}")]
    Parse { message: String },
    #[error("expected exactly one YAML document, got {actual}")]
    DocumentCount { actual: usize },
    #[error("YAML nesting exceeds {maximum}")]
    DepthLimit { maximum: usize },
    #[error("YAML tree exceeds {maximum} nodes")]
    NodeLimit { maximum: usize },
    #[error("YAML mapping keys must be strings")]
    NonStringMappingKey,
    #[error("duplicate YAML mapping key {0}")]
    DuplicateMappingKey(String),
    #[error("YAML aliases are not accepted in ae-sdd configuration")]
    AliasUnsupported,
    #[error("YAML loader returned an invalid value")]
    BadValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_rust2_loader_accepts_bounded_mapping() {
        let document =
            YamlDocument::parse(b"projectKey: ae-sdd\nselectors:\n  - source\n  - constraints\n")
                .expect("valid YAML");

        assert_eq!(
            document.get("projectKey").and_then(YamlValue::string),
            Some("ae-sdd")
        );
        assert_eq!(document.fingerprint(), document.fingerprint());
    }

    #[test]
    fn yaml_loader_fails_closed_on_multiple_documents_and_oversized_input() {
        assert!(matches!(
            YamlDocument::parse(b"one: 1\n---\ntwo: 2\n"),
            Err(YamlError::DocumentCount { actual: 2 })
        ));
        assert!(matches!(
            YamlDocument::parse_bounded(b"key: value", 2),
            Err(YamlError::ByteLimit { .. })
        ));
    }
}
