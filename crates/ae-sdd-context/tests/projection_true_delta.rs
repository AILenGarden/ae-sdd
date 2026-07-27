mod support;

use ae_sdd_context::{
    ContextProjection, ContextProjectionError, ContextView, MemoryVisibility, ProjectionBudget,
    ProjectionKind, RoleMemoryRef,
};
use ae_sdd_domain::{
    AgentRole, ArtifactDigest, ArtifactKind, ArtifactRef, ContextDigest, ContextRevision,
    DeliverableContract, DeliverableId, DeliverableRequirement, OperationId, ProjectPathScope,
    ProjectRelativePath,
};

use support::{artifact, delegation, root_projection_ids, session};

fn projection_from_view(
    revision: u64,
    view: ContextView,
    budget: ProjectionBudget,
) -> Result<ContextProjection, ContextProjectionError> {
    let ids = root_projection_ids(revision);
    ContextProjection::new(
        ids.projection_id,
        ids.session_id,
        ids.delegation_id,
        ids.role,
        ids.source_revision,
        ids.context_revision,
        ids.policy_digest,
        ids.inventory_generation,
        view,
        2_000,
        budget,
    )
}

fn projection(revision: u64, input_refs: Vec<ae_sdd_domain::ArtifactRef>) -> ContextProjection {
    let ids = root_projection_ids(revision);
    let view = ContextView::new(
        ids.session_id,
        ids.role,
        ids.delegation_id,
        format!("summary-{revision}"),
        input_refs,
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid view");
    projection_from_view(revision, view, ProjectionBudget::default()).expect("valid projection")
}

#[test]
fn delta_does_not_repeat_the_whole_context_view() {
    let previous = projection(
        1,
        vec![
            artifact("inputs/a.md", b"removed"),
            artifact("inputs/b.md", b"old"),
            artifact("inputs/stable.md", b"stable"),
        ],
    );
    let current = projection(
        2,
        vec![
            artifact("inputs/b.md", b"new"),
            artifact("inputs/c.md", b"added"),
            artifact("inputs/stable.md", b"stable"),
        ],
    );

    let response = current
        .response_from(
            Some(&previous),
            previous.context_revision(),
            previous.digest(),
        )
        .expect("delta response");

    assert_eq!(response.kind(), ProjectionKind::Delta);
    assert!(
        response.view().is_none(),
        "delta must not carry a full view"
    );
    let changes = response.changes().expect("explicit changes");
    assert_eq!(
        changes
            .changed_input_refs()
            .iter()
            .map(|reference| reference.path().as_str())
            .collect::<Vec<_>>(),
        vec!["inputs/b.md", "inputs/c.md"]
    );
    assert_eq!(
        changes
            .removed_input_paths()
            .iter()
            .map(ae_sdd_domain::ProjectRelativePath::as_str)
            .collect::<Vec<_>>(),
        vec!["inputs/a.md"]
    );
    assert!(
        !changes
            .changed_input_refs()
            .iter()
            .any(|reference| reference.path().as_str() == "inputs/stable.md")
    );
}

#[test]
fn projection_digest_is_independent_of_input_order() {
    let first = projection(
        3,
        vec![artifact("inputs/z.md", b"z"), artifact("inputs/a.md", b"a")],
    );
    let second = projection(
        3,
        vec![artifact("inputs/a.md", b"a"), artifact("inputs/z.md", b"z")],
    );

    assert_eq!(first.digest(), second.digest());
}

fn deliverable_contract() -> DeliverableContract {
    DeliverableContract::new(
        [DeliverableRequirement::new(
            DeliverableId::new("report").expect("valid deliverable id"),
            ArtifactKind::new("report").expect("valid artifact kind"),
            ProjectRelativePath::new("out/report.json").expect("valid path"),
        )],
        512,
        128,
    )
    .expect("valid deliverable contract")
}

fn rich_view(revision: u64, with_contract: bool) -> ContextView {
    let old = revision == 10;
    ContextView::new(
        session(1),
        AgentRole::Root,
        None,
        if old { "old summary" } else { "new summary" },
        if old {
            vec![
                artifact("inputs/changed.md", b"old"),
                artifact("inputs/removed.md", b"removed"),
            ]
        } else {
            vec![
                artifact("inputs/changed.md", b"new"),
                artifact("inputs/added.md", b"added"),
            ]
        },
        if old {
            vec![
                artifact("constraints/changed.md", b"old"),
                artifact("constraints/removed.md", b"removed"),
            ]
        } else {
            vec![
                artifact("constraints/changed.md", b"new"),
                artifact("constraints/added.md", b"added"),
            ]
        },
        if old {
            vec![
                RoleMemoryRef::new(
                    artifact("memory/changed.json", b"old"),
                    MemoryVisibility::RootSummary(session(1)),
                ),
                RoleMemoryRef::new(
                    artifact("memory/removed.json", b"removed"),
                    MemoryVisibility::Session(session(1)),
                ),
            ]
        } else {
            vec![
                RoleMemoryRef::new(
                    artifact("memory/changed.json", b"new"),
                    MemoryVisibility::RootSummary(session(1)),
                ),
                RoleMemoryRef::new(
                    artifact("memory/added.json", b"added"),
                    MemoryVisibility::Session(session(1)),
                ),
            ]
        },
        if old {
            vec![
                OperationId::new("state.keep").expect("valid operation"),
                OperationId::new("state.remove").expect("valid operation"),
            ]
        } else {
            vec![
                OperationId::new("state.keep").expect("valid operation"),
                OperationId::new("state.add").expect("valid operation"),
            ]
        },
        if old {
            vec![ProjectPathScope::Subtree(
                ProjectRelativePath::new("old").expect("valid path"),
            )]
        } else {
            vec![
                ProjectPathScope::ProjectRoot,
                ProjectPathScope::Subtree(ProjectRelativePath::new("new").expect("valid path")),
            ]
        },
        with_contract.then(deliverable_contract),
    )
    .expect("valid rich view")
}

#[test]
fn rich_delta_reports_every_changed_removed_and_authorization_dimension() {
    let previous = projection_from_view(10, rich_view(10, false), ProjectionBudget::default())
        .expect("previous projection");
    let current = projection_from_view(11, rich_view(11, true), ProjectionBudget::default())
        .expect("current projection");

    assert_eq!(
        current.projection_id(),
        root_projection_ids(11).projection_id
    );
    assert_eq!(current.session_id(), session(1));
    assert_eq!(current.delegation_id(), None);
    assert_eq!(current.role(), AgentRole::Root);
    assert_eq!(
        current.source_revision(),
        root_projection_ids(11).source_revision
    );
    assert_eq!(current.context_revision(), ContextRevision::new(11));
    assert_eq!(
        current.policy_digest(),
        root_projection_ids(11).policy_digest
    );
    assert_eq!(
        current.inventory_generation(),
        root_projection_ids(11).inventory_generation
    );
    assert_eq!(current.expires_at_unix_ms(), 2_000);
    assert!(current.byte_length() > 0);
    assert_eq!(current.view().summary(), "new summary");
    assert_eq!(current.view().input_refs().len(), 2);
    assert_eq!(current.view().constraint_refs().len(), 2);
    assert_eq!(current.view().memory_refs().len(), 2);
    assert_eq!(current.view().allowed_operations().len(), 2);
    assert_eq!(current.view().allowed_paths().len(), 2);
    assert_eq!(
        current
            .view()
            .deliverable_contract()
            .expect("deliverable contract")
            .max_result_bytes(),
        512
    );

    let response = current
        .response_from(
            Some(&previous),
            previous.context_revision(),
            previous.digest(),
        )
        .expect("rich delta response");
    assert_eq!(response.kind(), ProjectionKind::Delta);
    assert_eq!(response.base_revision(), previous.context_revision());
    assert_eq!(response.target_revision(), current.context_revision());
    assert_eq!(response.target_digest(), current.digest());
    assert!(response.view().is_none());
    let changes = response.changes().expect("explicit changes");
    assert_eq!(changes.summary(), Some("new summary"));
    assert_eq!(changes.changed_input_refs().len(), 2);
    assert_eq!(changes.removed_input_paths().len(), 1);
    assert_eq!(changes.changed_constraint_refs().len(), 2);
    assert_eq!(changes.removed_constraint_paths().len(), 1);
    assert_eq!(changes.changed_memory_refs().len(), 2);
    assert_eq!(changes.removed_memory_paths().len(), 1);
    assert_eq!(
        changes
            .added_operations()
            .iter()
            .map(OperationId::as_str)
            .collect::<Vec<_>>(),
        vec!["state.add"]
    );
    assert_eq!(
        changes
            .removed_operations()
            .iter()
            .map(OperationId::as_str)
            .collect::<Vec<_>>(),
        vec!["state.remove"]
    );
    assert_eq!(changes.added_paths().len(), 2);
    assert_eq!(changes.removed_paths().len(), 1);
    assert!(format!("{:?}", changes.deliverable_contract()).starts_with("Set("));

    let first_memory = &current.view().memory_refs()[0];
    assert!(
        first_memory
            .artifact()
            .path()
            .as_str()
            .starts_with("memory/")
    );
    assert!(matches!(
        first_memory.visibility(),
        MemoryVisibility::Session(_) | MemoryVisibility::RootSummary(_)
    ));

    let removed_contract =
        projection_from_view(12, rich_view(11, false), ProjectionBudget::default())
            .expect("projection without contract");
    let response = removed_contract
        .response_from(Some(&current), current.context_revision(), current.digest())
        .expect("contract removal delta");
    assert_eq!(
        format!(
            "{:?}",
            response
                .changes()
                .expect("delta changes")
                .deliverable_contract()
        ),
        "Removed"
    );
}

#[test]
fn memory_visibility_and_view_canonicalization_fail_closed() {
    let session_memory = RoleMemoryRef::new(
        artifact("memory/session.json", b"session"),
        MemoryVisibility::Session(session(2)),
    );
    assert!(session_memory.is_visible_to(session(2), AgentRole::Series, Some(delegation(1))));
    assert!(!session_memory.is_visible_to(session(1), AgentRole::Root, None));

    let delegation_memory = RoleMemoryRef::new(
        artifact("memory/delegation.json", b"delegation"),
        MemoryVisibility::Delegation(delegation(1)),
    );
    assert!(delegation_memory.is_visible_to(session(2), AgentRole::Series, Some(delegation(1))));
    assert!(!delegation_memory.is_visible_to(session(2), AgentRole::Series, Some(delegation(2))));

    let root_memory = RoleMemoryRef::new(
        artifact("memory/root.json", b"root"),
        MemoryVisibility::RootSummary(session(1)),
    );
    assert!(root_memory.is_visible_to(session(1), AgentRole::Root, None));
    assert!(!root_memory.is_visible_to(session(1), AgentRole::Series, Some(delegation(1))));

    assert!(matches!(
        ContextView::new(
            session(1),
            AgentRole::Root,
            None,
            "",
            vec![],
            vec![],
            vec![],
            [],
            [],
            None,
        ),
        Err(ContextProjectionError::EmptySummary)
    ));
    let duplicate = artifact("inputs/duplicate.md", b"same");
    assert!(matches!(
        ContextView::new(
            session(1),
            AgentRole::Root,
            None,
            "duplicate input",
            vec![duplicate.clone(), duplicate],
            vec![],
            vec![],
            [],
            [],
            None,
        ),
        Err(ContextProjectionError::DuplicateReferencePath)
    ));
    let duplicate_memory = RoleMemoryRef::new(
        artifact("memory/duplicate.json", b"same"),
        MemoryVisibility::Session(session(1)),
    );
    assert!(matches!(
        ContextView::new(
            session(1),
            AgentRole::Root,
            None,
            "duplicate memory",
            vec![],
            vec![],
            vec![duplicate_memory.clone(), duplicate_memory],
            [],
            [],
            None,
        ),
        Err(ContextProjectionError::DuplicateReferencePath)
    ));
}

#[test]
fn projection_identity_revision_and_delta_budgets_are_enforced() {
    assert!(matches!(
        ProjectionBudget::new(0, 1),
        Err(ContextProjectionError::InvalidBudget)
    ));
    assert!(matches!(
        ProjectionBudget::new(1, 0),
        Err(ContextProjectionError::InvalidBudget)
    ));
    let budget = ProjectionBudget::new(900, 700).expect("valid budget");
    assert_eq!(budget.maximum_for(AgentRole::Root), 900);
    assert_eq!(budget.maximum_for(AgentRole::Series), 700);

    let root_with_delegation = ContextView::new(
        session(1),
        AgentRole::Root,
        Some(delegation(1)),
        "root",
        vec![],
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid view shape");
    let ids = root_projection_ids(20);
    assert!(matches!(
        ContextProjection::new(
            ids.projection_id,
            ids.session_id,
            Some(delegation(1)),
            AgentRole::Root,
            ids.source_revision,
            ids.context_revision,
            ids.policy_digest,
            ids.inventory_generation,
            root_with_delegation,
            2_000,
            ProjectionBudget::default(),
        ),
        Err(ContextProjectionError::RootDelegationForbidden)
    ));

    let child_without_delegation = ContextView::new(
        session(2),
        AgentRole::Series,
        None,
        "child",
        vec![],
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid view shape");
    assert!(matches!(
        ContextProjection::new(
            ids.projection_id,
            session(2),
            None,
            AgentRole::Series,
            ids.source_revision,
            ids.context_revision,
            ids.policy_digest,
            ids.inventory_generation,
            child_without_delegation,
            2_000,
            ProjectionBudget::default(),
        ),
        Err(ContextProjectionError::ChildDelegationRequired)
    ));

    let valid_view = ContextView::new(
        session(1),
        AgentRole::Root,
        None,
        "root",
        vec![],
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid root view");
    assert!(matches!(
        ContextProjection::new(
            ids.projection_id,
            ids.session_id,
            None,
            AgentRole::Root,
            ids.source_revision,
            ids.context_revision,
            ids.policy_digest,
            ids.inventory_generation,
            valid_view.clone(),
            0,
            ProjectionBudget::default(),
        ),
        Err(ContextProjectionError::InvalidExpiry)
    ));
    let mismatched_view = ContextView::new(
        session(2),
        AgentRole::Root,
        None,
        "wrong target",
        vec![],
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid but mismatched view");
    assert!(matches!(
        ContextProjection::new(
            ids.projection_id,
            ids.session_id,
            None,
            AgentRole::Root,
            ids.source_revision,
            ids.context_revision,
            ids.policy_digest,
            ids.inventory_generation,
            mismatched_view,
            2_000,
            ProjectionBudget::default(),
        ),
        Err(ContextProjectionError::ViewIdentityMismatch)
    ));

    let current = projection_from_view(20, valid_view, ProjectionBudget::default())
        .expect("current projection");
    assert!(matches!(
        current.response_from(
            None,
            ContextRevision::new(21),
            ContextDigest::digest(b"future")
        ),
        Err(ContextProjectionError::RevisionStale)
    ));

    let previous = projection(30, vec![]);
    let huge_ref = ArtifactRef::new(
        ArtifactKind::new("input").expect("valid kind"),
        ProjectRelativePath::new("inputs/huge.bin").expect("valid path"),
        ArtifactDigest::digest(b"huge"),
        10_000,
    );
    let huge_view = ContextView::new(
        session(1),
        AgentRole::Root,
        None,
        "large logical delta",
        vec![huge_ref],
        vec![],
        vec![],
        [],
        [],
        None,
    )
    .expect("valid view");
    let current = projection_from_view(
        31,
        huge_view,
        ProjectionBudget::new(1_000, 1_000).expect("valid budget"),
    )
    .expect("projection metadata fits budget");
    assert!(matches!(
        current.response_from(
            Some(&previous),
            previous.context_revision(),
            previous.digest()
        ),
        Err(ContextProjectionError::DeltaBudgetExceeded { .. })
    ));
}
