#[test]
fn probe_can_be_claimed_completed_and_listed_without_exposing_lease() {
    let queue = RunnerJobQueue::default();
    let submitted = queue
        .submit_probe(None, "ping".to_string(), Some(30))
        .unwrap();
    let claim = queue
        .claim("lab-vm-01", 1, &["control_probe".to_string()])
        .unwrap()
        .unwrap();
    assert_eq!(claim.job_id, submitted.job_id);
    let completed = queue
        .complete(RunnerJobResultRequest {
            agent_id: "lab-vm-01".to_string(),
            job_id: claim.job_id.clone(),
            lease_token: claim.lease_token,
            state: RunnerJobResultState::Succeeded,
            message: Some("pong".to_string()),
            output: None,
        })
        .unwrap();
    assert_eq!(completed.state, RunnerJobState::Succeeded);
    assert_eq!(
        queue.inventory().jobs[0].result_message.as_deref(),
        Some("pong")
    );
}

#[test]
fn claimed_probe_requires_cancel_acknowledgement() {
    let queue = RunnerJobQueue::default();
    queue
        .submit_probe(Some("lab-vm-01".to_string()), "ping".to_string(), None)
        .unwrap();
    assert!(queue
        .claim("other-agent", 1, &["control_probe".to_string()])
        .unwrap()
        .is_none());
    let claim = queue
        .claim("lab-vm-01", 1, &["control_probe".to_string()])
        .unwrap()
        .unwrap();
    let cancelled = queue.cancel(&claim.job_id).unwrap();
    assert_eq!(cancelled.state, RunnerJobState::CancelRequested);
    let sync = queue
        .sync(
            "lab-vm-01",
            &[RunnerJobLeaseReference {
                job_id: claim.job_id.clone(),
                lease_token: claim.lease_token.clone(),
            }],
        )
        .unwrap();
    assert_eq!(sync.cancel_job_ids, vec![claim.job_id.clone()]);
    let invalid = queue.complete(RunnerJobResultRequest {
        agent_id: "lab-vm-01".to_string(),
        job_id: claim.job_id,
        lease_token: claim.lease_token,
        state: RunnerJobResultState::Succeeded,
        message: None,
        output: None,
    });
    assert!(matches!(invalid, Err(RunnerJobQueueError::Conflict(_))));
}

#[test]
fn claim_respects_agent_active_capacity() {
    let queue = RunnerJobQueue::default();
    queue.submit_probe(None, "first".to_string(), None).unwrap();
    queue
        .submit_probe(None, "second".to_string(), None)
        .unwrap();
    let capabilities = vec!["control_probe".to_string()];
    assert!(queue
        .claim("lab-vm-01", 1, &capabilities)
        .unwrap()
        .is_some());
    assert!(queue
        .claim("lab-vm-01", 1, &capabilities)
        .unwrap()
        .is_none());
}

#[test]
fn compile_checks_require_the_clang_capability_and_hide_source_from_inventory() {
    let queue = RunnerJobQueue::default();
    let submitted = queue
        .submit_compile_check(
            None,
            "int x(void) { return 0; }".to_string(),
            Some("lesson".to_string()),
            None,
        )
        .unwrap();
    assert_eq!(submitted.source_bytes, Some(25));
    assert!(queue
        .claim("probe-only", 1, &["control_probe".to_string()])
        .unwrap()
        .is_none());
    let claim = queue
        .claim("compiler", 1, &[COMPILE_CAPABILITY.to_string()])
        .unwrap()
        .unwrap();
    assert_eq!(claim.kind, COMPILE_JOB_KIND);
    assert_eq!(claim.program_name.as_deref(), Some("lesson"));
    assert!(claim
        .source
        .as_deref()
        .is_some_and(|source| source.contains("return")));
}

#[test]
fn user_compile_jobs_are_private_cancellable_and_quota_limited() {
    let queue = RunnerJobQueue::default();
    let submit = |owner: &str, suffix: u8| {
        queue.submit_user_compile_check(
            owner.to_string(),
            "compiler".to_string(),
            format!("int lesson_{suffix}(void) {{ return {suffix}; }}"),
            Some(format!("lesson-{suffix}")),
            None,
        )
    };
    let first = submit("alice", 1).unwrap();
    let second = submit("alice", 2).unwrap();
    assert_eq!(first.owner_username.as_deref(), Some("alice"));
    assert!(matches!(
        submit("alice", 3),
        Err(RunnerJobQueueError::Conflict(_))
    ));
    assert!(matches!(
        queue.job_for_owner(&first.job_id, "bob"),
        Err(RunnerJobQueueError::NotFound)
    ));
    assert!(matches!(
        queue.cancel_for_owner(&first.job_id, "bob"),
        Err(RunnerJobQueueError::NotFound)
    ));
    assert_eq!(
        queue.cancel_for_owner(&first.job_id, "alice").unwrap().state,
        RunnerJobState::Cancelled
    );
    assert_eq!(
        queue.cancel_for_owner(&second.job_id, "alice").unwrap().state,
        RunnerJobState::Cancelled
    );
}

#[test]
fn unclaimed_user_checks_expire_without_leaking_source_or_blocking_user_quota() {
    let queue = RunnerJobQueue::default();
    let submit = || queue.submit_user_compile_check("alice".into(), "compiler".into(),
        "int lesson(void) { return 0; }".into(), None, Some(20));
    let first = submit().unwrap();
    submit().unwrap();
    assert!(matches!(submit(), Err(RunnerJobQueueError::Conflict(_))));
    // No sleep or real Agent: simulate an abandoned browser's unclaimed jobs.
    for job in queue.jobs().values_mut() { job.created_at -= chrono::Duration::seconds(36); }
    let expired = queue.job_for_owner(&first.job_id, "alice").unwrap();
    assert_eq!(expired.state, RunnerJobState::Expired);
    assert!(expired.completed_at.is_some());
    assert!(queue.jobs().values().all(|job| job.source.is_none()));
    assert!(queue.claim("compiler", 2, &[COMPILE_CAPABILITY.into()]).unwrap().is_none());
    assert!(submit().is_ok(), "a lost cancellation must not consume quota forever");
    assert!(matches!(queue.job_for_owner(&first.job_id, "bob"), Err(RunnerJobQueueError::NotFound)));
}

#[test]
fn user_queue_expiry_is_inclusive_and_does_not_extend_terminal_retention() {
    let queue = RunnerJobQueue::default();
    let submitted = queue.submit_user_compile_check("alice".into(), "compiler".into(),
        "int lesson(void) { return 0; }".into(), None, Some(20)).unwrap();
    let now = submitted.created_at;
    let mut jobs = queue.jobs();
    reap(&mut jobs, now + chrono::Duration::seconds(34));
    assert_eq!(jobs[&submitted.job_id].state, RunnerJobState::Queued);
    let expiry = now + chrono::Duration::seconds(35);
    reap(&mut jobs, expiry);
    assert_eq!(jobs[&submitted.job_id].state, RunnerJobState::Expired);
    assert!(jobs[&submitted.job_id].source.is_none());
    reap(&mut jobs, expiry + chrono::Duration::seconds(10));
    assert_eq!(jobs[&submitted.job_id].completed_at, Some(expiry));
    reap(&mut jobs, expiry + chrono::Duration::seconds(901));
    assert!(jobs.is_empty());
}

#[test]
fn queue_wait_limit_does_not_shorten_claimed_leases_or_expire_staff_managed_work() {
    let queue = RunnerJobQueue::default();
    let user = queue.submit_user_compile_check("alice".into(), "compiler".into(),
        "int lesson(void) { return 0; }".into(), None, Some(20)).unwrap();
    queue.jobs().get_mut(&user.job_id).unwrap().created_at -= chrono::Duration::seconds(34);
    let claim = queue.claim("compiler", 1, &[COMPILE_CAPABILITY.into()]).unwrap().unwrap();
    let staff = queue.submit_compile_check(None, "int staff(void) { return 1; }".into(), None, None).unwrap();
    queue.jobs().get_mut(&staff.job_id).unwrap().created_at -= chrono::Duration::hours(1);
    let mut jobs = queue.jobs();
    reap(&mut jobs, claim.claimed_at + chrono::Duration::seconds(2));
    assert_eq!(jobs[&user.job_id].state, RunnerJobState::Claimed);
    assert_eq!(jobs[&user.job_id].deadline, Some(claim.deadline));
    assert_eq!(jobs[&staff.job_id].state, RunnerJobState::Queued);
    assert!(jobs[&staff.job_id].source.is_some());
    reap(&mut jobs, claim.deadline + chrono::Duration::seconds(1));
    assert_eq!(jobs[&user.job_id].state, RunnerJobState::Expired);
}
