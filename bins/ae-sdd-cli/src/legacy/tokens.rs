use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;
const MAX_VALUE_BYTES: usize = 64 * 1024;

/// Stable fail-closed argv error surfaced by the CLI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyArgumentError(String);

impl LegacyArgumentError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for LegacyArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for LegacyArgumentError {}

pub(super) fn validate_text(name: &str, value: String) -> Result<String, LegacyArgumentError> {
    if value.trim().is_empty()
        || value.len() > MAX_VALUE_BYTES
        || value.contains(['\0', '\r', '\n'])
    {
        Err(LegacyArgumentError::new(format!(
            "{name} is empty, contains control characters, or exceeds the value budget"
        )))
    } else {
        Ok(value)
    }
}

pub(super) fn kebab_to_camel(name: &str) -> Result<String, LegacyArgumentError> {
    let mut segments = name.split('-');
    let first = segments.next().unwrap_or_default();
    if !valid_segment(first) {
        return Err(invalid_flag(name));
    }
    let mut result = first.to_owned();
    for segment in segments {
        if !valid_segment(segment) {
            return Err(invalid_flag(name));
        }
        let mut characters = segment.chars();
        if let Some(first) = characters.next() {
            result.push(first.to_ascii_uppercase());
        }
        result.extend(characters);
    }
    Ok(result)
}

fn invalid_flag(name: &str) -> LegacyArgumentError {
    LegacyArgumentError::new(format!(
        "invalid --{name}; business flags must be lowercase kebab-case"
    ))
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

#[derive(Debug)]
pub(super) struct ParsedArguments {
    pub(super) options: BTreeMap<String, Option<String>>,
    pub(super) positionals: Vec<String>,
}

impl ParsedArguments {
    pub(super) fn parse(
        arguments: &[String],
        boolean_flags: &[&str],
    ) -> Result<Self, LegacyArgumentError> {
        Self::parse_with_repeatable(arguments, boolean_flags, &[])
    }

    pub(super) fn parse_with_repeatable(
        arguments: &[String],
        boolean_flags: &[&str],
        repeatable_flags: &[&str],
    ) -> Result<Self, LegacyArgumentError> {
        let total_bytes = arguments.iter().map(String::len).sum::<usize>();
        if total_bytes > MAX_ARGUMENT_BYTES {
            return Err(LegacyArgumentError::new(
                "legacy argv exceeds the one-megabyte budget",
            ));
        }
        let booleans: BTreeSet<_> = boolean_flags.iter().copied().collect();
        let repeatable: BTreeSet<_> = repeatable_flags.iter().copied().collect();
        let mut options = BTreeMap::new();
        let mut repeated = BTreeMap::<String, Vec<String>>::new();
        let mut positionals = Vec::new();
        let mut index = 0;
        while index < arguments.len() {
            let token = &arguments[index];
            if !token.starts_with("--") {
                if token.starts_with('-') {
                    return Err(LegacyArgumentError::new(format!(
                        "unsupported short or malformed option {token}"
                    )));
                }
                validate_text("positional argument", token.clone())?;
                positionals.push(token.clone());
                index += 1;
                continue;
            }
            if token == "--" {
                return Err(LegacyArgumentError::new(
                    "the -- positional escape is not accepted by legacy adapters",
                ));
            }
            let option = &token[2..];
            let (name, inline) = option
                .split_once('=')
                .map_or((option, None), |(name, value)| (name, Some(value)));
            kebab_to_camel(name)?;
            let value = if let Some(value) = inline {
                Some(validate_text(name, value.to_owned())?)
            } else if booleans.contains(name) {
                None
            } else if arguments
                .get(index + 1)
                .is_some_and(|next| !next.starts_with("--"))
            {
                index += 1;
                Some(validate_text(name, arguments[index].clone())?)
            } else {
                None
            };
            if repeatable.contains(name) {
                let value = value.ok_or_else(|| {
                    LegacyArgumentError::new(format!("--{name} requires a value"))
                })?;
                repeated.entry(name.to_owned()).or_default().push(value);
            } else if options.insert(name.to_owned(), value).is_some() {
                return Err(LegacyArgumentError::new(format!(
                    "duplicate legacy flag --{name}"
                )));
            }
            index += 1;
        }
        for (name, values) in repeated {
            let encoded = serde_json::to_string(&values).map_err(|_| {
                LegacyArgumentError::new(format!("repeatable --{name} values are invalid"))
            })?;
            options.insert(name, Some(encoded));
        }
        Ok(Self {
            options,
            positionals,
        })
    }

    pub(super) fn take_aliases(
        &mut self,
        aliases: &[&str],
    ) -> Result<Option<String>, LegacyArgumentError> {
        let present: Vec<_> = aliases
            .iter()
            .filter(|alias| self.options.contains_key(**alias))
            .copied()
            .collect();
        if present.len() > 1 {
            return Err(LegacyArgumentError::new(format!(
                "ambiguous aliases were supplied together: {}",
                present
                    .iter()
                    .map(|name| format!("--{name}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        present
            .first()
            .map_or(Ok(None), |name| self.take_required_optional(name))
    }

    pub(super) fn take_required_optional(
        &mut self,
        name: &str,
    ) -> Result<Option<String>, LegacyArgumentError> {
        match self.options.remove(name) {
            Some(Some(value)) => Ok(Some(value)),
            Some(None) => Err(LegacyArgumentError::new(format!(
                "--{name} requires a value"
            ))),
            None => Ok(None),
        }
    }

    pub(super) fn take_boolean(&mut self, name: &str) -> Result<bool, LegacyArgumentError> {
        match self.options.remove(name) {
            None => Ok(false),
            Some(None) => Ok(true),
            Some(Some(value)) => match value.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(LegacyArgumentError::new(format!(
                    "--{name} accepts only true or false when assigned"
                ))),
            },
        }
    }
}
