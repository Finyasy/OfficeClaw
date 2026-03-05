#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Cancelled,
    Executed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalEvent {
    Approve,
    Reject,
    Cancel,
    Expire,
    ExecuteSuccess,
    ExecuteFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineError {
    pub message: &'static str,
}

pub fn transition(state: ApprovalStatus, event: ApprovalEvent) -> Result<ApprovalStatus, StateMachineError> {
    use ApprovalEvent::*;
    use ApprovalStatus::*;

    let next = match (state, event) {
        (Pending, Approve) => Approved,
        (Pending, Reject) => Rejected,
        (Pending, Cancel) => Cancelled,
        (Pending, Expire) => Expired,
        (Approved, ExecuteSuccess) => Executed,
        (Approved, ExecuteFailure) => Failed,
        _ => {
            return Err(StateMachineError {
                message: "invalid approval transition",
            })
        }
    };

    Ok(next)
}
