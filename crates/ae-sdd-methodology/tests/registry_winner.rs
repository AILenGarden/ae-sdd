use ae_sdd_contracts::{OverrideDisposition, OverrideLayer, SkillId};
use ae_sdd_domain::{ArtifactDigest, ProjectRelativePath};
use ae_sdd_methodology::{
    OverrideAuthorization, RegistryCandidate, RegistryTraceReason, RegistryViolation,
    resolve_registry,
};

fn candidate(
    name: &str,
    target: &str,
    layer: OverrideLayer,
    authorization: OverrideAuthorization,
) -> RegistryCandidate {
    RegistryCandidate::new(
        SkillId::new(name).unwrap(),
        ProjectRelativePath::new(target).unwrap(),
        layer,
        ArtifactDigest::digest(format!("source:{layer:?}:{name}")),
        ArtifactDigest::digest(format!("content:{name}")),
        authorization,
    )
    .unwrap()
}

fn permutations<T: Clone>(values: &[T]) -> Vec<Vec<T>> {
    if values.is_empty() {
        return vec![Vec::new()];
    }
    let mut result = Vec::new();
    for index in 0..values.len() {
        let mut remaining = values.to_vec();
        let head = remaining.remove(index);
        for mut tail in permutations(&remaining) {
            tail.insert(0, head.clone());
            result.push(tail);
        }
    }
    result
}

#[test]
fn registry_winner_is_permutation_stable_and_content_bound() {
    let target = ProjectRelativePath::new("source/skills/phase2-coding/coding-skill.md").unwrap();
    let candidates = vec![
        candidate(
            "repository-coding",
            target.as_str(),
            OverrideLayer::Repository,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "project-coding",
            target.as_str(),
            OverrideLayer::Project,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "global-coding",
            target.as_str(),
            OverrideLayer::Global,
            OverrideAuthorization::Authorized,
        ),
    ];
    let expected = resolve_registry(candidates.clone()).unwrap();
    for permutation in permutations(&candidates) {
        assert_eq!(resolve_registry(permutation).unwrap(), expected);
    }

    let winner = expected.winner_for(&target).unwrap().candidate();
    assert_eq!(winner.layer(), OverrideLayer::Project);
    assert_eq!(winner.name().as_str(), "project-coding");
    assert_eq!(
        winner.content_digest(),
        ArtifactDigest::digest(b"content:project-coding")
    );
    assert_eq!(
        expected
            .trace()
            .iter()
            .map(|item| (item.layer(), item.disposition()))
            .collect::<Vec<_>>(),
        vec![
            (OverrideLayer::Project, OverrideDisposition::Selected),
            (OverrideLayer::Global, OverrideDisposition::Shadowed),
            (OverrideLayer::Repository, OverrideDisposition::Shadowed),
        ]
    );
}

#[test]
fn registry_conflicts_and_unauthorized_candidates_fail_closed_with_full_trace() {
    let same_name = vec![
        candidate(
            "duplicate",
            "skill-one",
            OverrideLayer::Project,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "duplicate",
            "skill-two",
            OverrideLayer::Project,
            OverrideAuthorization::Authorized,
        ),
    ];
    let expected = resolve_registry(same_name.clone()).unwrap_err();
    for permutation in permutations(&same_name) {
        assert_eq!(resolve_registry(permutation).unwrap_err(), expected);
    }
    assert!(matches!(
        expected.violations(),
        [RegistryViolation::SameLayerNameConflict { .. }]
    ));
    assert_eq!(expected.trace().len(), 2);

    let same_target = vec![
        candidate(
            "first",
            "shared-target",
            OverrideLayer::Global,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "second",
            "shared-target",
            OverrideLayer::Global,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "repository",
            "shared-target",
            OverrideLayer::Repository,
            OverrideAuthorization::Authorized,
        ),
    ];
    let error = resolve_registry(same_target).unwrap_err();
    assert!(
        error.violations().iter().any(|violation| matches!(
            violation,
            RegistryViolation::SameLayerTargetConflict { .. }
        ))
    );
    assert_eq!(error.trace().len(), 3);

    let unauthorized = vec![
        candidate(
            "project",
            "shared-target",
            OverrideLayer::Project,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "global",
            "shared-target",
            OverrideLayer::Global,
            OverrideAuthorization::Denied,
        ),
    ];
    let error = resolve_registry(unauthorized).unwrap_err();
    assert!(matches!(
        error.violations(),
        [RegistryViolation::Unauthorized { .. }]
    ));
    assert_eq!(error.trace().len(), 2);
    assert_eq!(
        error
            .trace()
            .iter()
            .find(|item| item.name().as_str() == "global")
            .unwrap()
            .reason(),
        RegistryTraceReason::Unauthorized
    );
}

#[test]
fn cross_layer_name_and_target_overrides_each_select_the_highest_layer() {
    let project_target = ProjectRelativePath::new("project-target").unwrap();
    let repository_target = ProjectRelativePath::new("repository-target").unwrap();
    let candidates = vec![
        candidate(
            "shared-name",
            repository_target.as_str(),
            OverrideLayer::Repository,
            OverrideAuthorization::Authorized,
        ),
        candidate(
            "shared-name",
            project_target.as_str(),
            OverrideLayer::Project,
            OverrideAuthorization::Authorized,
        ),
    ];
    let expected = resolve_registry(candidates.clone()).unwrap();
    for permutation in permutations(&candidates) {
        assert_eq!(resolve_registry(permutation).unwrap(), expected);
    }
    assert_eq!(expected.winners().len(), 1);
    assert!(expected.winner_for(&project_target).is_some());
    assert!(expected.winner_for(&repository_target).is_none());
    assert_eq!(
        expected
            .trace()
            .iter()
            .map(|item| (item.layer(), item.disposition()))
            .collect::<Vec<_>>(),
        vec![
            (OverrideLayer::Project, OverrideDisposition::Selected),
            (OverrideLayer::Repository, OverrideDisposition::Shadowed),
        ]
    );
}
