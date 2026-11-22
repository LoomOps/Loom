//! Pure Loom decisions. No transport, database, filesystem, async runtime or JSON.
pub mod item;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError(pub &'static str);
impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for DomainError {}
pub type Result<T> = std::result::Result<T, DomainError>;
macro_rules! identity {
    ($($name:ident),+) => {$ (
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value=value.into();
                if value.is_empty() || value.len()>2048 || value.chars().any(char::is_control) {
                    return Err(DomainError("Invalid identity"));
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {f.write_str(&self.0)} }
    )+};
}
identity!(
    JobId,
    AttemptId,
    TicketId,
    ScenarioId,
    EntityId,
    PrincipalId,
    NodeId,
    StepId,
    StoreId,
    OperationKey
);
macro_rules! vocabulary {
    ($name:ident {$($variant:ident => $text:literal),+ $(,)?}) => {
        #[derive(Debug,Clone,Copy,PartialEq,Eq)] pub enum $name {$($variant),+}
        impl $name {
            pub fn as_str(self)-> &'static str {match self {$(Self::$variant => $text),+}}
            pub fn parse(value:&str)->Result<Self> {match value {$($text=>Ok(Self::$variant)),+, _=>Err(DomainError(concat!("Unknown ",stringify!($name))))}}
        }
    };
}
vocabulary!(JobState {Queued=>"queued",Preparing=>"preparing",Ready=>"ready",Running=>"running",Processing=>"processing",Complete=>"complete",Failed=>"failed",Interrupted=>"interrupted",Cancelled=>"cancelled",Blocked=>"blocked",ResultFailed=>"result_failed"});
vocabulary!(Stage {Prepare=>"prepare",Run=>"run",Finalize=>"finalize",OnComplete=>"on_complete",OnError=>"on_error",OnCancel=>"on_cancel"});
vocabulary!(Purpose {Research=>"research",Operations=>"operations"});
vocabulary!(PresentationRole {Main=>"main",Support=>"support",Test=>"test"});
vocabulary!(Lane {Cpu=>"cpu",Gpu=>"gpu"});
vocabulary!(RecoveryState {Waiting=>"waiting",Pending=>"pending",Claimed=>"claimed",Executing=>"executing",Deferred=>"deferred",Exhausted=>"exhausted",Closed=>"closed"});
vocabulary!(EffectState {Pending=>"pending",Accepted=>"accepted",Unknown=>"unknown",Returned=>"returned",Rejected=>"rejected"});
impl JobState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Complete | Self::Failed | Self::Interrupted | Self::Cancelled
        )
    }
    pub fn public(self) -> &'static str {
        match self {
            Self::Complete => "completed",
            Self::Processing => "finalizing",
            Self::ResultFailed => "finalizing_failed",
            Self::Blocked => "queued",
            Self::Interrupted => "failed",
            other => other.as_str(),
        }
    }
    pub fn holds_work(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Ready | Self::Running | Self::Processing | Self::ResultFailed
        )
    }
}
#[derive(Debug, Clone)]
pub struct Fence {
    pub attempt: Option<AttemptId>,
    pub revision: u64,
    pub epoch: u64,
}
impl Fence {
    pub fn matches(&self, other: &Self) -> bool {
        self.attempt == other.attempt
            && self.revision == other.revision
            && self.epoch == other.epoch
    }
}
#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub state: JobState,
    pub fence: Fence,
    pub cancel_requested: bool,
}
#[derive(Debug, Clone, Copy)]
pub enum Event {
    Prepare,
    Prepared,
    Start,
    Returned { success: bool },
    RequestCancel,
    ReturnCancelled,
    Publish,
    PublicationFailed,
    Fail,
    Interrupt,
    RetryExecution,
    RetryPublication,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub state: JobState,
    pub cancel_requested: bool,
}
impl Job {
    pub fn decide(&self, expected: &Fence, event: Event) -> Result<Decision> {
        if !self.fence.matches(expected) {
            return Err(DomainError("Stale execution fence"));
        }
        let state = self.state;
        let mut cancelled = self.cancel_requested;
        let target = match event {
            Event::RequestCancel if !matches!(state, JobState::Complete | JobState::Cancelled) => {
                cancelled = true;
                if matches!(
                    state,
                    JobState::Queued
                        | JobState::Blocked
                        | JobState::Ready
                        | JobState::Failed
                        | JobState::Interrupted
                        | JobState::ResultFailed
                ) {
                    JobState::Cancelled
                } else {
                    state
                }
            }
            Event::ReturnCancelled if cancelled && !state.terminal() => JobState::Cancelled,
            Event::Prepare if state == JobState::Queued && !cancelled => JobState::Preparing,
            Event::Prepared if state == JobState::Preparing && !cancelled => JobState::Ready,
            Event::Start if matches!(state, JobState::Queued | JobState::Ready) && !cancelled => {
                JobState::Running
            }
            Event::Returned { success }
                if matches!(state, JobState::Running | JobState::Preparing) =>
            {
                if cancelled {
                    JobState::Cancelled
                } else if success {
                    JobState::Processing
                } else {
                    JobState::Failed
                }
            }
            Event::Publish if state == JobState::Processing => {
                if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Complete
                }
            }
            Event::PublicationFailed if state == JobState::Processing => {
                if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::ResultFailed
                }
            }
            Event::Fail if !state.terminal() => {
                if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Failed
                }
            }
            Event::Interrupt
                if matches!(
                    state,
                    JobState::Running | JobState::Preparing | JobState::Ready
                ) =>
            {
                if cancelled {
                    JobState::Cancelled
                } else {
                    JobState::Interrupted
                }
            }
            Event::RetryExecution
                if matches!(
                    state,
                    JobState::Failed | JobState::Interrupted | JobState::Cancelled
                ) =>
            {
                cancelled = false;
                JobState::Queued
            }
            Event::RetryPublication if state == JobState::ResultFailed => {
                cancelled = false;
                JobState::Processing
            }
            _ => return Err(DomainError("Job transition is not permitted")),
        };
        Ok(Decision {
            state: target,
            cancel_requested: cancelled,
        })
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Readiness {
    pub admission_active: bool,
    pub operator_paused: bool,
    pub held: bool,
    pub dependency_pending: bool,
    pub dependency_failed: bool,
    pub recovery_reserved: bool,
    pub runtime_validation: bool,
    pub lane_busy: bool,
    pub delayed: bool,
    pub output_edit_blocked: Option<&'static str>,
}
impl Readiness {
    pub fn reason(self) -> Option<&'static str> {
        if !self.admission_active {
            Some("admission_fenced")
        } else if self.operator_paused {
            Some("operator_paused")
        } else if self.held {
            Some("held")
        } else if let Some(reason) = self.output_edit_blocked {
            Some(reason)
        } else if self.dependency_pending || self.dependency_failed {
            Some("dependencies")
        } else if self.recovery_reserved {
            Some("recovery_reserved")
        } else if self.runtime_validation {
            Some("runtime_validation")
        } else if self.delayed {
            Some("backoff")
        } else if self.lane_busy {
            Some("lane_busy")
        } else {
            None
        }
    }
}
pub fn dependency_available(state: JobState, artifact_ready: bool) -> bool {
    state == JobState::Complete
        || artifact_ready && matches!(state, JobState::Processing | JobState::ResultFailed)
}
#[derive(Debug, Clone)]
pub struct RecoveryLease {
    pub state: RecoveryState,
    pub rounds: u32,
    pub agent: Option<PrincipalId>,
    pub token: Option<String>,
    pub deadline: Option<f64>,
    pub hard_deadline: Option<f64>,
}
impl RecoveryLease {
    pub fn can_claim(&self, now: f64) -> Result<()> {
        if self.rounds >= 3 {
            return Err(DomainError("Recovery budget exhausted"));
        }
        if self.state == RecoveryState::Claimed && self.deadline.is_some_and(|d| d > now) {
            return Err(DomainError("Recovery already claimed"));
        }
        if self.state == RecoveryState::Closed {
            return Err(DomainError("No open recovery incident"));
        }
        Ok(())
    }
    pub fn authorize(&self, agent: &PrincipalId, token: &str, now: f64) -> Result<()> {
        if self.state != RecoveryState::Claimed
            || self.agent.as_ref() != Some(agent)
            || self.token.as_deref() != Some(token)
            || !self.deadline.is_some_and(|d| d > now)
            || !self.hard_deadline.is_some_and(|d| d > now)
        {
            return Err(DomainError("Recovery lease is not current"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct PlanNode {
    pub id: NodeId,
    pub dependencies: Vec<NodeId>,
}
pub fn order_plan(nodes: &[PlanNode]) -> Result<Vec<NodeId>> {
    if nodes.is_empty() || nodes.len() > 64 {
        return Err(DomainError("Plan requires 1..64 nodes"));
    }
    let map: BTreeMap<_, _> = nodes.iter().map(|n| (n.id.clone(), n)).collect();
    if map.len() != nodes.len() {
        return Err(DomainError("Duplicate execution node"));
    }
    let mut done = BTreeSet::new();
    let mut result = Vec::new();
    while done.len() < nodes.len() {
        let available: Vec<_> = map
            .iter()
            .filter(|(id, n)| {
                !done.contains(*id) && n.dependencies.iter().all(|d| done.contains(d))
            })
            .map(|(id, _)| id.clone())
            .collect();
        if available.is_empty() {
            return Err(DomainError("Unknown dependency or execution cycle"));
        }
        for id in available {
            done.insert(id.clone());
            result.push(id);
        }
    }
    Ok(result)
}
/// Wall-clock elapsed time is separate from the union of actual active stages.
pub fn union_intervals(values: &[(f64, f64)]) -> Result<Vec<(f64, f64)>> {
    let mut sorted = values.to_vec();
    if sorted
        .iter()
        .any(|(a, b)| !a.is_finite() || !b.is_finite() || b < a)
    {
        return Err(DomainError("Invalid execution interval"));
    }
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = vec![];
    for (start, end) in sorted {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    Ok(merged)
}
#[cfg(test)]
#[path = "../../../../tests/rust/domain.rs"]
mod tests;
