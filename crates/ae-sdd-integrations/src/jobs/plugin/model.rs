use std::collections::BTreeMap;

use ae_sdd_methodology::OverrideAuthorization;
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct Layer {
    pub(super) label: &'static str,
    pub(super) priority: u8,
    pub(super) relative: String,
    pub(super) exists: bool,
    pub(super) digest: Option<String>,
    pub(super) plugins: Vec<Plugin>,
    pub(super) errors: Vec<String>,
}

#[derive(Clone)]
pub(super) struct Plugin {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) version: String,
    pub(super) author: Option<String>,
    pub(super) description: String,
    pub(super) path: String,
    pub(super) replaces: Option<String>,
    pub(super) provides: Option<String>,
    pub(super) dependencies: Vec<String>,
    pub(super) compatibility: Option<String>,
    pub(super) tags: Vec<String>,
    pub(super) resolved_path: Option<String>,
    pub(super) content_digest: Option<String>,
    pub(super) authorization: OverrideAuthorization,
}

impl Plugin {
    pub(super) fn target(&self) -> Option<&str> {
        self.replaces.as_deref().or(self.provides.as_deref())
    }

    pub(super) fn value(&self) -> Value {
        json!({
            "name":self.name,
            "type":self.kind,
            "version":self.version,
            "author":self.author,
            "description":self.description,
            "path":self.path,
            "replaces":self.replaces,
            "provides":self.provides,
            "dependencies":self.dependencies,
            "compatibility":self.compatibility.as_ref().map(|range| json!({"aeSddVersion":range})),
            "tags":self.tags,
            "resolvedPath":self.resolved_path,
            "contentDigest":self.content_digest,
        })
    }
}

pub(super) struct Resolution {
    pub(super) winners: BTreeMap<String, (usize, usize)>,
    pub(super) conflicts: Vec<Value>,
    pub(super) override_traces: BTreeMap<String, Vec<Value>>,
    pub(super) registry_digest: String,
    pub(super) adapter_errors: Vec<String>,
}

pub(super) struct ResolvedPluginFile {
    pub(super) relative: String,
    pub(super) content_digest: String,
}
