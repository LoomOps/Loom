use super::*;
fn job(state: JobState) -> Job {
    Job {
        id: JobId::new("old-job-id").unwrap(),
        state,
        fence: Fence {
            attempt: Some(AttemptId::new("attempt-1").unwrap()),
            revision: 1,
            epoch: 1,
        },
        cancel_requested: false,
    }
}
#[test]
fn late_attempt_is_rejected() {
    let j = job(JobState::Running);
    let mut old = j.fence.clone();
    old.attempt = Some(AttemptId::new("old").unwrap());
    assert!(j.decide(&old, Event::Returned { success: true }).is_err());
}
#[test]
fn cancel_wins_publication() {
    let mut j = job(JobState::Processing);
    j.cancel_requested = true;
    assert_eq!(
        j.decide(&j.fence, Event::Publish).unwrap().state,
        JobState::Cancelled
    );
}
#[test]
fn finalization_retry_has_no_execution_transition() {
    let j = job(JobState::ResultFailed);
    assert_eq!(
        j.decide(&j.fence, Event::RetryPublication).unwrap().state,
        JobState::Processing
    );
    assert!(j.decide(&j.fence, Event::Start).is_err());
}
#[test]
fn stale_artifact_cannot_admit_cancelled_dependency() {
    assert!(!dependency_available(JobState::Cancelled, true));
    assert!(dependency_available(JobState::ResultFailed, true));
}
#[test]
fn dag_rejects_cycle_and_unknown_nodes() {
    let id = NodeId::new("a").unwrap();
    assert!(order_plan(&[PlanNode {
        id: id.clone(),
        dependencies: vec![id]
    }])
    .is_err());
}
#[test]
fn queue_purpose_cannot_hide_occupancy() {
    let r = Readiness {
        admission_active: true,
        ..Readiness::default()
    };
    assert_eq!(r.reason(), None);
}
#[test]
fn parallel_time_is_not_double_counted() {
    assert_eq!(
        union_intervals(&[(10., 20.), (15., 30.), (40., 45.)]).unwrap(),
        vec![(10., 30.), (40., 45.)]
    );
}
#[test]
fn terminal_state_is_absorbing_for_callbacks() {
    let j = job(JobState::Complete);
    for e in [
        Event::Publish,
        Event::Fail,
        Event::Returned { success: true },
        Event::Start,
    ] {
        assert!(j.decide(&j.fence, e).is_err());
    }
}
