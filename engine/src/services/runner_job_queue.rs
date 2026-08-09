use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::runner_job::{
    RunnerJobClaim, RunnerJobInventory, RunnerJobLeaseReference, RunnerJobResultRequest,
    RunnerJobResultState, RunnerJobState, RunnerJobSyncResponse, RunnerJobView,
};

const JOB_KIND: &str = "control_probe";
const MAX_JOBS: usize = 512;
const TERMINAL_RETENTION: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub struct RunnerJobQueue {
    inner: Arc<Mutex<HashMap<String, JobRecord>>>,
}

struct JobRecord {
    job_id: String,
    state: RunnerJobState,
    target_agent_id: Option<String>,
    assigned_agent_id: Option<String>,
    message: String,
    timeout_seconds: u64,
    lease_token: Option<String>,
    result_message: Option<String>,
    output: Option<String>,
    created_at: DateTime<Utc>,
    claimed_at: Option<DateTime<Utc>>,
    deadline: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerJobQueueError {
    Invalid(String),
    NotFound,
    Conflict(String),
    Capacity,
}

impl RunnerJobQueue {
    pub fn submit_probe(
        &self,
        target_agent_id: Option<String>,
        message: String,
        timeout_seconds: Option<u64>,
    ) -> Result<RunnerJobView, RunnerJobQueueError> {
        let message = message.trim().to_string();
        if message.is_empty() || message.len() > 512 || message.chars().any(char::is_control) {
            return invalid("probe message must contain 1-512 printable characters");
        }
        if target_agent_id
            .as_ref()
            .is_some_and(|value| value.is_empty())
        {
            return invalid("target agent id cannot be empty");
        }
        let timeout_seconds = timeout_seconds.unwrap_or(30).clamp(5, 300);
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        if jobs.len() >= MAX_JOBS {
            return Err(RunnerJobQueueError::Capacity);
        }
        let job_id = format!("job-{}", Uuid::new_v4().simple());
        let record = JobRecord {
            job_id: job_id.clone(),
            state: RunnerJobState::Queued,
            target_agent_id,
            assigned_agent_id: None,
            message,
            timeout_seconds,
            lease_token: None,
            result_message: None,
            output: None,
            created_at: now,
            claimed_at: None,
            deadline: None,
            completed_at: None,
        };
        let view = view(&record);
        jobs.insert(job_id, record);
        Ok(view)
    }

    pub fn claim(
        &self,
        agent_id: &str,
        max_active_jobs: usize,
    ) -> Result<Option<RunnerJobClaim>, RunnerJobQueueError> {
        if max_active_jobs == 0 || max_active_jobs > 32 {
            return invalid("runner agent claim capacity must be between 1 and 32");
        }
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        let active_jobs = jobs
            .values()
            .filter(|job| {
                job.assigned_agent_id.as_deref() == Some(agent_id)
                    && matches!(
                        job.state,
                        RunnerJobState::Claimed | RunnerJobState::CancelRequested
                    )
            })
            .count();
        if active_jobs >= max_active_jobs {
            return Ok(None);
        }
        let selected = jobs
            .values()
            .filter(|job| {
                job.state == RunnerJobState::Queued
                    && job
                        .target_agent_id
                        .as_deref()
                        .is_none_or(|target| target == agent_id)
            })
            .min_by_key(|job| job.created_at)
            .map(|job| job.job_id.clone());
        let Some(job_id) = selected else {
            return Ok(None);
        };
        let job = jobs.get_mut(&job_id).expect("selected job still exists");
        let lease_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let deadline = now + chrono::Duration::seconds(job.timeout_seconds as i64);
        job.state = RunnerJobState::Claimed;
        job.assigned_agent_id = Some(agent_id.to_string());
        job.lease_token = Some(lease_token.clone());
        job.claimed_at = Some(now);
        job.deadline = Some(deadline);
        Ok(Some(RunnerJobClaim {
            job_id,
            kind: JOB_KIND,
            message: job.message.clone(),
            lease_token,
            claimed_at: now,
            deadline,
        }))
    }

    pub fn cancel(&self, job_id: &str) -> Result<RunnerJobView, RunnerJobQueueError> {
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        let job = jobs.get_mut(job_id).ok_or(RunnerJobQueueError::NotFound)?;
        match job.state {
            RunnerJobState::Queued => {
                job.state = RunnerJobState::Cancelled;
                job.completed_at = Some(now);
            }
            RunnerJobState::Claimed => job.state = RunnerJobState::CancelRequested,
            RunnerJobState::CancelRequested => {}
            _ => {
                return Err(RunnerJobQueueError::Conflict(
                    "terminal runner jobs cannot be cancelled".to_string(),
                ))
            }
        }
        Ok(view(job))
    }

    pub fn sync(
        &self,
        agent_id: &str,
        leases: &[RunnerJobLeaseReference],
    ) -> Result<RunnerJobSyncResponse, RunnerJobQueueError> {
        if leases.len() > 32 {
            return invalid("job sync accepts at most 32 leases");
        }
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        let mut cancel_job_ids = Vec::new();
        let mut lost_job_ids = Vec::new();
        for lease in leases {
            let Some(job) = jobs.get(&lease.job_id) else {
                lost_job_ids.push(lease.job_id.clone());
                continue;
            };
            if !lease_matches(job, agent_id, &lease.lease_token) {
                lost_job_ids.push(lease.job_id.clone());
            } else if job.state == RunnerJobState::CancelRequested {
                cancel_job_ids.push(lease.job_id.clone());
            } else if !matches!(job.state, RunnerJobState::Claimed) {
                lost_job_ids.push(lease.job_id.clone());
            }
        }
        Ok(RunnerJobSyncResponse {
            cancel_job_ids,
            lost_job_ids,
        })
    }

    pub fn complete(
        &self,
        request: RunnerJobResultRequest,
    ) -> Result<RunnerJobView, RunnerJobQueueError> {
        validate_result(&request)?;
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        let job = jobs
            .get_mut(&request.job_id)
            .ok_or(RunnerJobQueueError::NotFound)?;
        if !lease_matches(job, &request.agent_id, &request.lease_token) {
            return Err(RunnerJobQueueError::Conflict(
                "runner job lease is invalid".to_string(),
            ));
        }
        if !matches!(
            job.state,
            RunnerJobState::Claimed | RunnerJobState::CancelRequested
        ) {
            return Err(RunnerJobQueueError::Conflict(
                "runner job is no longer active".to_string(),
            ));
        }
        if job.state == RunnerJobState::CancelRequested
            && request.state != RunnerJobResultState::Cancelled
        {
            return Err(RunnerJobQueueError::Conflict(
                "runner job cancellation must be acknowledged".to_string(),
            ));
        }
        job.state = match request.state {
            RunnerJobResultState::Succeeded => RunnerJobState::Succeeded,
            RunnerJobResultState::Failed => RunnerJobState::Failed,
            RunnerJobResultState::Cancelled => RunnerJobState::Cancelled,
        };
        job.result_message = trim_optional(request.message);
        job.output = trim_optional(request.output);
        job.completed_at = Some(now);
        Ok(view(job))
    }

    pub fn inventory(&self) -> RunnerJobInventory {
        let now = Utc::now();
        let mut jobs = self.jobs();
        reap(&mut jobs, now);
        let mut views = jobs.values().map(view).collect::<Vec<_>>();
        views.sort_by_key(|job| job.created_at);
        RunnerJobInventory {
            generated_at: now,
            total_jobs: views.len(),
            jobs: views,
        }
    }

    fn jobs(&self) -> MutexGuard<'_, HashMap<String, JobRecord>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn validate_result(request: &RunnerJobResultRequest) -> Result<(), RunnerJobQueueError> {
    if request.agent_id.is_empty() || request.job_id.is_empty() || request.lease_token.len() != 64 {
        return invalid("runner job result identity or lease is invalid");
    }
    if request
        .message
        .as_ref()
        .is_some_and(|value| value.len() > 512 || value.chars().any(char::is_control))
    {
        return invalid("runner job result message exceeds 512 printable characters");
    }
    if request
        .output
        .as_ref()
        .is_some_and(|value| value.len() > 16_384)
    {
        return invalid("runner job output exceeds 16384 bytes");
    }
    if request.output.as_ref().is_some_and(|value| {
        value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return invalid("runner job output contains unsupported control characters");
    }
    Ok(())
}

fn lease_matches(job: &JobRecord, agent_id: &str, lease_token: &str) -> bool {
    job.assigned_agent_id.as_deref() == Some(agent_id)
        && job.lease_token.as_deref() == Some(lease_token)
}

fn reap(jobs: &mut HashMap<String, JobRecord>, now: DateTime<Utc>) {
    for job in jobs.values_mut() {
        if matches!(
            job.state,
            RunnerJobState::Claimed | RunnerJobState::CancelRequested
        ) && job.deadline.is_some_and(|deadline| now > deadline)
        {
            job.state = RunnerJobState::Expired;
            job.completed_at = Some(now);
        }
    }
    let retention = chrono::Duration::from_std(TERMINAL_RETENTION).unwrap();
    jobs.retain(|_, job| {
        job.completed_at
            .is_none_or(|completed_at| now - completed_at <= retention)
    });
}

fn view(job: &JobRecord) -> RunnerJobView {
    RunnerJobView {
        job_id: job.job_id.clone(),
        kind: JOB_KIND,
        state: job.state,
        target_agent_id: job.target_agent_id.clone(),
        assigned_agent_id: job.assigned_agent_id.clone(),
        message: job.message.clone(),
        timeout_seconds: job.timeout_seconds,
        result_message: job.result_message.clone(),
        output: job.output.clone(),
        created_at: job.created_at,
        claimed_at: job.claimed_at,
        deadline: job.deadline,
        completed_at: job.completed_at,
    }
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn invalid<T>(message: &str) -> Result<T, RunnerJobQueueError> {
    Err(RunnerJobQueueError::Invalid(message.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::runner_job::{RunnerJobLeaseReference, RunnerJobResultRequest};

    #[test]
    fn probe_can_be_claimed_completed_and_listed_without_exposing_lease() {
        let queue = RunnerJobQueue::default();
        let submitted = queue
            .submit_probe(None, "ping".to_string(), Some(30))
            .unwrap();
        let claim = queue.claim("lab-vm-01", 1).unwrap().unwrap();
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
        assert!(queue.claim("other-agent", 1).unwrap().is_none());
        let claim = queue.claim("lab-vm-01", 1).unwrap().unwrap();
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
        assert!(queue.claim("lab-vm-01", 1).unwrap().is_some());
        assert!(queue.claim("lab-vm-01", 1).unwrap().is_none());
    }
}
