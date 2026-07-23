mod support;

use ae_sdd_context::{
    ContextProjection, ContextProjectionError, ContextView, ProjectionBudget, ProjectionKind,
};

use support::root_projection_ids;

fn projection(
    revision: u64,
    summary: &str,
    budget: ProjectionBudget,
) -> Result<ContextProjection, ContextProjectionError> {
    let ids = root_projection_ids(revision);
    let view = ContextView::new(
        ids.session_id,
        ids.role,
        ids.delegation_id,
        summary,
        vec![],
        vec![],
        vec![],
        [],
        [],
        None,
    )?;
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

#[test]
fn projection_returns_no_change_delta_or_full_and_enforces_budget() {
    let previous =
        projection(1, "first", ProjectionBudget::default()).expect("previous projection");
    let current = projection(2, "second", ProjectionBudget::default()).expect("current projection");

    assert_eq!(
        current
            .response_from(
                Some(&previous),
                current.context_revision(),
                current.digest()
            )
            .expect("no-change response")
            .kind(),
        ProjectionKind::NoChange
    );
    assert_eq!(
        current
            .response_from(
                Some(&previous),
                previous.context_revision(),
                previous.digest()
            )
            .expect("delta response")
            .kind(),
        ProjectionKind::Delta
    );
    assert_eq!(
        current
            .response_from(None, previous.context_revision(), previous.digest())
            .expect("full response")
            .kind(),
        ProjectionKind::Full
    );

    let tiny = ProjectionBudget::new(1, 1).expect("non-zero budget");
    assert!(matches!(
        projection(3, "cannot fit", tiny),
        Err(ContextProjectionError::BudgetExceeded { .. })
    ));
}
