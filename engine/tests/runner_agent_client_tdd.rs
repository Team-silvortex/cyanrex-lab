use std::{collections::BTreeMap, time::Duration};

use cyanrex_engine::{
    build_router, build_state,
    models::{runner_agent::RunnerAgentIsolation, runner_job::RunnerJobState},
    services::runner_agent_client::{RunnerAgentClient, RunnerAgentClientConfig},
};

const TOKEN: &str = "runner-agent-client-integration-token-32-bytes";

#[tokio::test]
#[ignore = "requires a loopback TCP listener"]
async fn runner_agent_client_completes_a_signed_control_probe_over_http() {
    std::env::set_var("CYANREX_RUNNER_AGENT_TOKEN", TOKEN);
    let state = build_state();
    std::env::remove_var("CYANREX_RUNNER_AGENT_TOKEN");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_router(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut labels = BTreeMap::new();
    labels.insert("test".to_string(), "integration".to_string());
    let config = RunnerAgentClientConfig {
        engine_url: format!("http://{address}"),
        bootstrap_token: TOKEN.to_string(),
        agent_id: "client-test-agent".to_string(),
        isolation: RunnerAgentIsolation::SharedKernel,
        max_concurrent: 1,
        capabilities: vec!["control_probe".to_string()],
        compile_check: None,
        labels,
        poll_interval: Duration::from_millis(10),
        request_timeout: Duration::from_secs(5),
        run_once: true,
    };
    let mut client = RunnerAgentClient::new(config).unwrap();
    let agent = client.register().await.unwrap();
    assert_eq!(agent.agent_id, "client-test-agent");

    let submitted = state
        .runner_job_queue
        .submit_probe(
            Some("client-test-agent".to_string()),
            "integration ping".to_string(),
            Some(30),
        )
        .unwrap();
    let completed = client.poll_once().await.unwrap().unwrap();
    assert_eq!(completed.job_id, submitted.job_id);
    assert_eq!(completed.state, RunnerJobState::Succeeded);
    assert!(completed
        .output
        .as_deref()
        .is_some_and(|output| output.contains("integration ping")));

    let inventory = state.runner_job_queue.inventory();
    assert_eq!(inventory.jobs[0].state, RunnerJobState::Succeeded);
    server.abort();
}
