mod support;

use ae_sdd_context::{ContextProjectionError, ContextView, MemoryVisibility, RoleMemoryRef};
use ae_sdd_domain::{AgentRole, OperationId, ProjectPathScope};

use support::{artifact, delegation, session};

#[test]
fn private_child_memory_cannot_enter_root_projection() {
    let child_private = RoleMemoryRef::new(
        artifact(".ae-sdd/memory/child.json", b"child private"),
        MemoryVisibility::Session(session(2)),
    );
    let root_view = ContextView::new(
        session(1),
        AgentRole::Root,
        None,
        "root orchestration summary",
        vec![],
        vec![],
        vec![child_private],
        [OperationId::new("state.next_actions").expect("valid operation")],
        [ProjectPathScope::ProjectRoot],
        None,
    );
    assert!(matches!(
        root_view,
        Err(ContextProjectionError::MemoryVisibilityViolation)
    ));

    let child_view = ContextView::new(
        session(2),
        AgentRole::Series,
        Some(delegation(1)),
        "series assignment",
        vec![],
        vec![],
        vec![RoleMemoryRef::new(
            artifact(".ae-sdd/memory/series.json", b"series private"),
            MemoryVisibility::Delegation(delegation(1)),
        )],
        [],
        [ProjectPathScope::ProjectRoot],
        None,
    );
    assert!(child_view.is_ok());
}
