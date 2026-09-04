use crate::smoke_http::SmokeHttpClient;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub struct Options {
    pub engine_url: String,
    pub origin: String,
    pub agent_id: String,
    pub password: String,
    pub totp_secret: String,
    pub ready_attempts: u32,
    pub ready_interval: Duration,
    pub job_attempts: u32,
    pub job_interval: Duration,
}

#[derive(Deserialize)]
struct BackendInventory {
    agents: Vec<Backend>,
}

#[derive(Deserialize)]
struct Backend {
    agent_id: String,
}

#[derive(Deserialize)]
struct SubmittedJob {
    job_id: String,
}

#[derive(Deserialize)]
struct JobStatus {
    state: String,
    message: String,
    result: Option<JobResult>,
}

#[derive(Deserialize)]
struct JobResult {
    ok: bool,
}

pub async fn run(options: &Options) -> Result<String, String> {
    validate_options(options)?;
    println!("[cyanrex] Logging in for the user-scoped remote-check smoke test...");
    let client = SmokeHttpClient::login(
        &options.engine_url,
        &options.origin,
        &options.password,
        &options.totp_secret,
    )
    .await?;
    println!(
        "[cyanrex] Waiting for compile Agent '{}'...",
        options.agent_id
    );
    wait_for_agent(&client, options).await?;
    let submitted: SubmittedJob = client
        .post_json(
            "/ebpf/check/remote",
            &json!({
                "agent_id": options.agent_id,
                "program_name": "agent-smoke",
                "code": "int cyanrex_agent_smoke(void) { return 0; }\n",
            }),
        )
        .await?;
    if submitted.job_id.is_empty() || submitted.job_id.contains(['\r', '\n']) {
        return Err("remote compiler returned an invalid job ID".to_owned());
    }
    println!(
        "[cyanrex] Waiting for remote compile job {}...",
        submitted.job_id
    );
    let result = wait_for_job(&client, options, &submitted.job_id).await;
    if let Err((error, cleanup_needed)) = result {
        if !cleanup_needed {
            return Err(error);
        }
        let cleanup = cancel_job(&client, &submitted.job_id).await;
        return match cleanup {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(format!(
                "{error}; additionally failed to cancel {}: {cleanup_error}",
                submitted.job_id
            )),
        };
    }
    Ok(submitted.job_id)
}

async fn wait_for_agent(client: &SmokeHttpClient, options: &Options) -> Result<(), String> {
    for attempt in 0..options.ready_attempts {
        let inventory: BackendInventory = client.get_json("/ebpf/check/backends", &[]).await?;
        if inventory
            .agents
            .iter()
            .any(|agent| agent.agent_id == options.agent_id)
        {
            return Ok(());
        }
        if attempt + 1 < options.ready_attempts {
            tokio::time::sleep(options.ready_interval).await;
        }
    }
    Err(format!(
        "compile Agent '{}' did not become available",
        options.agent_id
    ))
}

async fn wait_for_job(
    client: &SmokeHttpClient,
    options: &Options,
    job_id: &str,
) -> Result<(), (String, bool)> {
    for attempt in 0..options.job_attempts {
        let status: JobStatus = client
            .get_json("/ebpf/check/remote", &[("job_id", job_id)])
            .await
            .map_err(|error| (error, true))?;
        match status.state.as_str() {
            "succeeded" if status.result.is_some_and(|result| result.ok) => return Ok(()),
            "succeeded" => {
                return Err((
                    "remote compiler returned a non-successful result".to_owned(),
                    false,
                ))
            }
            "failed" | "cancelled" | "expired" => {
                return Err((
                    format!(
                        "remote compile job ended as '{}': {}",
                        status.state, status.message
                    ),
                    false,
                ))
            }
            "queued" | "claimed" | "cancel_requested" => {}
            state => {
                return Err((
                    format!("remote compile job returned invalid state {state:?}"),
                    true,
                ))
            }
        }
        if attempt + 1 < options.job_attempts {
            tokio::time::sleep(options.job_interval).await;
        }
    }
    Err(("remote compile job timed out".to_owned(), true))
}

async fn cancel_job(client: &SmokeHttpClient, job_id: &str) -> Result<(), String> {
    let _: Value = client
        .post_json("/ebpf/check/remote/cancel", &json!({"job_id": job_id}))
        .await?;
    Ok(())
}

fn validate_options(options: &Options) -> Result<(), String> {
    if !(3..=64).contains(&options.agent_id.len())
        || !options
            .agent_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err("runner Agent ID must be a 3-64 character safe identifier".to_owned());
    }
    if options.password.is_empty() || options.password.contains(['\r', '\n']) {
        return Err("CYANREX_ADMIN_PASSWORD must be a non-empty single-line value".to_owned());
    }
    if options.ready_attempts == 0 || options.job_attempts == 0 {
        return Err("runner Agent smoke polling attempts must be positive".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "smoke_runner_agent_tests.rs"]
mod tests;
