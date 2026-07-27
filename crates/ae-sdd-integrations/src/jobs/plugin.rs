mod model;
mod parser;
mod render;
mod resolution;

use ae_sdd_runtime::RuntimeResult;
use serde_json::Value;

use super::common::JobContext;
use render::{list, trace, validate};
use resolution::{load_layers, resolve};

pub(super) fn execute(
    context: &JobContext<'_>,
    entrypoint: &str,
    arguments: &Value,
) -> RuntimeResult<Value> {
    let layers = load_layers(context)?;
    let resolution = resolve(&layers);
    match entrypoint {
        "plugin.list" => Ok(list(&layers, &resolution)),
        "plugin.validate" => Ok(validate(&layers, &resolution)),
        "plugin.trace" => trace(context, &layers, &resolution, arguments),
        _ => Err(super::common::schema_error("unsupported plugin entrypoint")),
    }
}

#[cfg(test)]
mod tests;
