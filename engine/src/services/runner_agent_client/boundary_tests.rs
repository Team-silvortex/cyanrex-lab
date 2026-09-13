use super::*;
use crate::services::runner_job_queue::RunnerJobQueue;
use axum::{extract::State, routing::post, Json, Router};
use std::sync::{Arc, Mutex};

#[path = "response_tests.rs"]
mod response_tests;

struct TestServer {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn start(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, task }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone)]
struct LeaseStub {
    queue: RunnerJobQueue,
    lost: bool,
    cancelled: bool,
    results: Arc<Mutex<Vec<RunnerJobResultRequest>>>,
}

async fn sync_stub(
    State(state): State<LeaseStub>,
    Json(request): Json<RunnerJobSyncRequest>,
) -> Json<RunnerJobSyncResponse> {
    let ids: Vec<_> = request
        .leases
        .iter()
        .map(|lease| lease.job_id.clone())
        .collect();
    Json(RunnerJobSyncResponse {
        lost_job_ids: if state.lost { ids.clone() } else { vec![] },
        cancel_job_ids: if state.cancelled { ids } else { vec![] },
    })
}

async fn result_stub(
    State(state): State<LeaseStub>,
    Json(request): Json<RunnerJobResultRequest>,
) -> Json<RunnerJobView> {
    state.results.lock().unwrap().push(request.clone());
    Json(state.queue.complete(request).unwrap())
}

async fn exercise_lease(lost: bool, cancelled: bool) {
    let queue = RunnerJobQueue::default();
    queue
        .submit_probe(None, "synthetic probe".into(), Some(30))
        .unwrap();
    let job = queue
        .claim("synthetic-agent", 1, &["control_probe".into()])
        .unwrap()
        .unwrap();
    if cancelled {
        queue.cancel(&job.job_id).unwrap();
    }
    let results = Arc::new(Mutex::new(Vec::new()));
    let server = TestServer::start(
        Router::new()
            .route("/runner/agent/jobs/sync", post(sync_stub))
            .route("/runner/agent/jobs/result", post(result_stub))
            .with_state(LeaseStub {
                queue,
                lost,
                cancelled,
                results: results.clone(),
            }),
    )
    .await;
    let mut client = RunnerAgentClient::new(RunnerAgentClientConfig {
        engine_url: server.url.clone(),
        bootstrap_token: "synthetic-unused-bootstrap-token".into(),
        agent_id: "synthetic-agent".into(),
        isolation: RunnerAgentIsolation::Container,
        max_concurrent: 1,
        capabilities: vec!["control_probe".into()],
        compile_check: None,
        labels: BTreeMap::new(),
        poll_interval: Duration::from_millis(10),
        request_timeout: Duration::from_secs(3),
        run_once: true,
    })
    .unwrap();
    client.credential = Some("synthetic-credential-for-loopback-tests".into());
    let outcome = client.process_job(&job).await;
    let posted = results.lock().unwrap();
    if lost {
        assert!(
            posted.is_empty(),
            "lost leases must not execute/report or acknowledge cancellation"
        );
        let error = outcome.unwrap_err();
        assert!(
            error.is_conflict() && error.retryable(),
            "discard this job but keep polling: {error}"
        );
    } else {
        let completed = outcome.unwrap();
        assert_eq!(posted.len(), 1);
        if cancelled {
            assert_eq!(
                completed.state,
                crate::models::runner_job::RunnerJobState::Cancelled
            );
            assert!(posted[0].output.is_none());
        } else {
            assert_eq!(
                completed.state,
                crate::models::runner_job::RunnerJobState::Succeeded
            );
            assert!(posted[0]
                .output
                .as_ref()
                .unwrap()
                .contains("synthetic probe"));
        }
    }
}

#[tokio::test]
async fn lost_lease_prevents_execution_and_result_submission() {
    exercise_lease(true, false).await;
}

#[tokio::test]
async fn lost_lease_takes_precedence_over_a_cancel_instruction() {
    exercise_lease(true, true).await;
}

#[tokio::test]
async fn valid_and_cancelled_leases_keep_their_existing_completion_behavior() {
    exercise_lease(false, false).await;
    exercise_lease(false, true).await;
}
