use super::manifest::embedded_routes;
use super::model::{LegacyCommandRoute, LegacyRouteError, ResolvedLegacyCommand};

/// Resolve one exact frozen command id.
pub fn resolve_command_id(command_id: &str) -> Result<LegacyCommandRoute, LegacyRouteError> {
    embedded_routes()?
        .iter()
        .find(|route| route.command_id == command_id)
        .cloned()
        .ok_or_else(|| LegacyRouteError::UnknownOrRemovedDeprecated(command_id.to_owned()))
}

/// Resolve the longest frozen command prefix and preserve all remaining argv.
///
/// Matching is exact and case-sensitive. No normalization or fuzzy fallback is
/// allowed because that could silently grant a different authorization scope.
pub fn resolve_legacy_argv(args: &[String]) -> Result<ResolvedLegacyCommand, LegacyRouteError> {
    if args.is_empty() {
        return Err(LegacyRouteError::MissingCommand);
    }
    let route = embedded_routes()?
        .iter()
        .filter_map(|candidate| {
            let token_count = candidate.command_tokens().count();
            let matches = args.len() >= token_count
                && candidate
                    .command_tokens()
                    .zip(args.iter())
                    .all(|(expected, actual)| expected == actual);
            matches.then_some((token_count, candidate))
        })
        .max_by_key(|(token_count, _)| *token_count)
        .map(|(token_count, route)| (token_count, route.clone()));

    let Some((consumed_arguments, route)) = route else {
        return Err(LegacyRouteError::UnknownOrRemovedDeprecated(args.join(" ")));
    };
    if let super::model::LegacyTarget::Rejected {
        stable_code,
        remediation,
    } = &route.target
    {
        return Err(LegacyRouteError::RemovedDeprecated {
            command_id: route.command_id,
            stable_code: stable_code.clone(),
            remediation: remediation.clone(),
        });
    }
    Ok(ResolvedLegacyCommand {
        route,
        consumed_arguments,
        trailing_arguments: args[consumed_arguments..].to_vec(),
    })
}
