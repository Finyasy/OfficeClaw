use agent_core::policy::state_machine::{transition, ApprovalEvent, ApprovalStatus};

#[test]
fn pending_to_approved_is_valid() {
    let next = transition(ApprovalStatus::Pending, ApprovalEvent::Approve).unwrap();
    assert_eq!(next, ApprovalStatus::Approved);
}

#[test]
fn pending_to_expired_is_valid() {
    let next = transition(ApprovalStatus::Pending, ApprovalEvent::Expire).unwrap();
    assert_eq!(next, ApprovalStatus::Expired);
}

#[test]
fn approved_to_executed_is_valid() {
    let next = transition(ApprovalStatus::Approved, ApprovalEvent::ExecuteSuccess).unwrap();
    assert_eq!(next, ApprovalStatus::Executed);
}

#[test]
fn approved_to_failed_is_valid() {
    let next = transition(ApprovalStatus::Approved, ApprovalEvent::ExecuteFailure).unwrap();
    assert_eq!(next, ApprovalStatus::Failed);
}

#[test]
fn cannot_execute_from_pending() {
    let err = transition(ApprovalStatus::Pending, ApprovalEvent::ExecuteSuccess).unwrap_err();
    assert_eq!(err.message, "invalid approval transition");
}

#[test]
fn terminal_states_cannot_transition() {
    let rejected = transition(ApprovalStatus::Rejected, ApprovalEvent::Approve);
    let cancelled = transition(ApprovalStatus::Cancelled, ApprovalEvent::Approve);
    let expired = transition(ApprovalStatus::Expired, ApprovalEvent::Approve);
    assert!(rejected.is_err());
    assert!(cancelled.is_err());
    assert!(expired.is_err());
}
