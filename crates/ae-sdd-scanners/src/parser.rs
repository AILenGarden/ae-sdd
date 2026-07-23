use ae_sdd_domain::ProjectRelativePath;
use ae_sdd_inventory::{YamlDocument, YamlError};
use quick_xml::{Reader, events::Event};
use thiserror::Error;

const MAX_XML_DEPTH: usize = 128;
const MAX_XML_EVENTS: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserKind {
    Rust,
    Python,
    JavaScript,
    Jvm,
    Xml,
    Yaml,
    Markdown,
    PlainText,
}

/// Central extension-to-parser registry shared by every scanner.
pub struct SourceParserRegistry;

impl SourceParserRegistry {
    pub fn parser_for(path: &ProjectRelativePath) -> Option<ParserKind> {
        let name = path.as_str().rsplit('/').next()?;
        let extension = name.rsplit_once('.')?.1;
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Some(ParserKind::Rust),
            "py" => Some(ParserKind::Python),
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => Some(ParserKind::JavaScript),
            "java" | "kt" | "kts" | "groovy" => Some(ParserKind::Jvm),
            "xml" => Some(ParserKind::Xml),
            "yaml" | "yml" => Some(ParserKind::Yaml),
            "md" | "mdx" => Some(ParserKind::Markdown),
            "toml" | "properties" | "json" | "txt" | "sh" | "ps1" => Some(ParserKind::PlainText),
            _ => None,
        }
    }

    pub fn validate(
        parser: ParserKind,
        path: &ProjectRelativePath,
        input: &[u8],
    ) -> Result<(), ParseError> {
        match parser {
            ParserKind::Yaml => {
                YamlDocument::parse(input).map_err(|source| ParseError::Yaml {
                    path: path.clone(),
                    source,
                })?;
                Ok(())
            }
            ParserKind::Xml => validate_xml(path, input),
            _ => std::str::from_utf8(input)
                .map(|_| ())
                .map_err(|_| ParseError::NotUtf8(path.clone())),
        }
    }
}

fn validate_xml(path: &ProjectRelativePath, input: &[u8]) -> Result<(), ParseError> {
    let text = std::str::from_utf8(input).map_err(|_| ParseError::NotUtf8(path.clone()))?;
    let mut reader = Reader::from_str(text);
    let mut depth = 0_usize;
    let mut events = 0_usize;
    let mut seen_root = false;
    let mut finished_root = false;
    loop {
        events += 1;
        if events > MAX_XML_EVENTS {
            return Err(ParseError::Xml {
                path: path.clone(),
                message: format!("document exceeds {MAX_XML_EVENTS} parser events"),
            });
        }
        match reader.read_event() {
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    if seen_root || finished_root {
                        return Err(ParseError::Xml {
                            path: path.clone(),
                            message: "document has multiple root elements".to_owned(),
                        });
                    }
                    seen_root = true;
                }
                depth += 1;
                if depth > MAX_XML_DEPTH {
                    return Err(ParseError::Xml {
                        path: path.clone(),
                        message: format!("nesting exceeds {MAX_XML_DEPTH}"),
                    });
                }
            }
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    if seen_root || finished_root {
                        return Err(ParseError::Xml {
                            path: path.clone(),
                            message: "document has multiple root elements".to_owned(),
                        });
                    }
                    seen_root = true;
                    finished_root = true;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.checked_sub(1).ok_or_else(|| ParseError::Xml {
                    path: path.clone(),
                    message: "unexpected closing element".to_owned(),
                })?;
                if depth == 0 {
                    finished_root = true;
                }
            }
            Ok(Event::DocType(_) | Event::GeneralRef(_)) => {
                return Err(ParseError::Xml {
                    path: path.clone(),
                    message: "DTD and entity references are not accepted".to_owned(),
                });
            }
            Ok(Event::Text(value))
                if depth == 0 && !value.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(ParseError::Xml {
                    path: path.clone(),
                    message: "text outside the root element".to_owned(),
                });
            }
            Ok(Event::CData(_)) if depth == 0 => {
                return Err(ParseError::Xml {
                    path: path.clone(),
                    message: "CDATA outside the root element".to_owned(),
                });
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(ParseError::Xml {
                    path: path.clone(),
                    message: error.to_string(),
                });
            }
        }
    }
    if !seen_root || depth != 0 {
        return Err(ParseError::Xml {
            path: path.clone(),
            message: "document has no complete root element".to_owned(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("scanner has no registered parser for: {0}")]
    UnsupportedPath(ProjectRelativePath),
    #[error("scanner input is not UTF-8: {0}")]
    NotUtf8(ProjectRelativePath),
    #[error("invalid YAML scanner input {path}: {source}")]
    Yaml {
        path: ProjectRelativePath,
        source: YamlError,
    },
    #[error("invalid XML scanner input {path}: {message}")]
    Xml {
        path: ProjectRelativePath,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_parser_registry_uses_bounded_yaml_rust2_loader() {
        let path = ProjectRelativePath::new(".ae-sdd/config.yaml").expect("valid path");
        assert_eq!(
            SourceParserRegistry::parser_for(&path),
            Some(ParserKind::Yaml)
        );
        assert!(SourceParserRegistry::validate(ParserKind::Yaml, &path, b"key: value\n").is_ok());
        assert!(
            SourceParserRegistry::validate(ParserKind::Yaml, &path, b"key: [unterminated\n")
                .is_err()
        );
    }

    #[test]
    fn xml_parser_rejects_malformed_and_dtd_inputs() {
        let path = ProjectRelativePath::new("target/test-results/TEST-suite.xml").expect("path");
        assert!(SourceParserRegistry::validate(ParserKind::Xml, &path, b"<testsuite/>").is_ok());
        assert!(SourceParserRegistry::validate(ParserKind::Xml, &path, b"<testsuite>").is_err());
        assert!(
            SourceParserRegistry::validate(
                ParserKind::Xml,
                &path,
                b"<!DOCTYPE foo [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><foo>&x;</foo>",
            )
            .is_err()
        );
    }
}
