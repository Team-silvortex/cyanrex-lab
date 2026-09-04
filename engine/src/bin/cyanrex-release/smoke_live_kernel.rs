use crate::evidence;
use crate::smoke_http::SmokeHttpClient;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

const TEMPLATE_ID: &str = "ringbuf-hi-freq-sampler";

pub struct Options {
    pub engine_url: String,
    pub origin: String,
    pub password: String,
    pub totp_secret: String,
    pub stream_seconds: u32,
    pub poll_attempts: u32,
    pub poll_interval: Duration,
    pub report: Option<PathBuf>,
    pub release_metadata: Option<PathBuf>,
}

#[derive(Debug)]
pub struct Outcome {
    pub program_name: String,
    pub pin_path: String,
    pub report: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Attachments {
    pin_paths: Vec<String>,
}

#[derive(Deserialize)]
struct Template {
    id: String,
    code: String,
}

#[derive(Deserialize)]
struct RunResponse {
    success: bool,
    stage: String,
    message: String,
    pin_path: Option<String>,
}

#[derive(Deserialize)]
struct DetachResponse {
    ok: bool,
    clean: bool,
    detached: Vec<String>,
}

pub async fn run(options: &Options) -> Result<Outcome, String> {
    validate_options(options)?;
    println!("[cyanrex] Authenticating privileged live-kernel acceptance...");
    let client = SmokeHttpClient::login(
        &options.engine_url,
        &options.origin,
        &options.password,
        &options.totp_secret,
    )
    .await?;
    let environment: Value = client.get_json("/helper/environment", &[]).await?;
    evidence::validate_environment(&environment)
        .map_err(|error| format!("live kernel environment report is invalid: {error}"))?;
    require_empty_attachments(&client, "before acceptance").await?;
    let templates: Vec<Template> = client.get_json("/ebpf/templates", &[]).await?;
    let template = templates
        .into_iter()
        .find(|template| template.id == TEMPLATE_ID && !template.code.is_empty())
        .ok_or_else(|| format!("required live kernel template is missing: {TEMPLATE_ID}"))?;
    let program_name = format!(
        "release-kernel-smoke-{}",
        &Uuid::new_v4().simple().to_string()[..16]
    );
    println!("[cyanrex] Loading and attaching the release kernel smoke program...");
    let run_result: Result<RunResponse, String> = client
        .post_json(
            "/ebpf/run",
            &json!({
                "code": template.code,
                "template_id": TEMPLATE_ID,
                "program_name": program_name,
                "runtime_backend": "aya",
                "sampling_per_sec": 20,
                "stream_seconds": options.stream_seconds,
                "enable_kernel_stream": true,
            }),
        )
        .await;
    let response = match run_result {
        Ok(response) => response,
        Err(error) => return Err(with_cleanup(&client, None, error).await),
    };
    if !response.success || response.stage != "run" {
        return Err(with_cleanup(
            &client,
            None,
            format!("live kernel run failed: {}", response.message),
        )
        .await);
    }
    let Some(pin_path) = response.pin_path.filter(|path| {
        path.starts_with("/sys/fs/bpf/") && !path.contains(['\r', '\n']) && path.len() <= 4_096
    }) else {
        return Err(with_cleanup(
            &client,
            None,
            "live kernel run did not return a safe bpffs pin path".to_owned(),
        )
        .await);
    };
    let event = match accept_attached_program(&client, options, &program_name, &pin_path).await {
        Ok(event) => event,
        Err(error) => return Err(with_cleanup(&client, Some(&pin_path), error).await),
    };
    let report = if let Some(output) = options.report.as_deref() {
        Some(evidence::create_from_values(
            environment,
            event,
            &program_name,
            &pin_path,
            options.release_metadata.as_deref(),
            output,
        )?)
    } else {
        None
    };
    Ok(Outcome {
        program_name,
        pin_path,
        report,
    })
}

async fn accept_attached_program(
    client: &SmokeHttpClient,
    options: &Options,
    program_name: &str,
    pin_path: &str,
) -> Result<Value, String> {
    let attached: Attachments = client.get_json("/ebpf/attachments", &[]).await?;
    if !attached.pin_paths.iter().any(|path| path == pin_path) {
        return Err("live kernel attachment is not tracked after a successful run".to_owned());
    }
    println!("[cyanrex] Waiting for a real kernel ringbuf event...");
    let event = poll_event(client, options, program_name).await?;
    let detached: DetachResponse = client
        .post_json("/ebpf/detach", &json!({"pin_path": pin_path}))
        .await?;
    if !detached.ok || !detached.clean || !detached.detached.iter().any(|path| path == pin_path) {
        return Err("live kernel detach did not report a clean exact-path removal".to_owned());
    }
    require_empty_attachments(client, "after cleanup").await?;
    Ok(event)
}

async fn poll_event(
    client: &SmokeHttpClient,
    options: &Options,
    program_name: &str,
) -> Result<Value, String> {
    for attempt in 0..options.poll_attempts {
        let events: Vec<Value> = client
            .get_json(
                "/events",
                &[
                    ("category", "kernel"),
                    ("since_minutes", "5"),
                    ("limit", "500"),
                ],
            )
            .await?;
        if let Some(event) = events.into_iter().find(|event| {
            event["event_type"].as_str() == Some("ebpf.kernel_ringbuf")
                && event["payload"]["program_name"].as_str() == Some(program_name)
                && event["payload"]["bytes"]
                    .as_u64()
                    .is_some_and(|bytes| bytes > 0)
        }) {
            return Ok(event);
        }
        if attempt + 1 < options.poll_attempts {
            tokio::time::sleep(options.poll_interval).await;
        }
    }
    Err("no live kernel ringbuf event reached the authenticated event history".to_owned())
}

async fn require_empty_attachments(client: &SmokeHttpClient, stage: &str) -> Result<(), String> {
    let attachments: Attachments = client.get_json("/ebpf/attachments", &[]).await?;
    if attachments.pin_paths.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "live kernel acceptance requires an empty administrator attachment set {stage}"
        ))
    }
}

async fn with_cleanup(client: &SmokeHttpClient, pin_path: Option<&str>, error: String) -> String {
    let cleanup: Result<DetachResponse, String> = client
        .post_json("/ebpf/detach", &json!({"pin_path": pin_path}))
        .await;
    match cleanup {
        Ok(response)
            if response.ok
                && response.clean
                && pin_path.is_none_or(|pin| response.detached.iter().any(|item| item == pin)) =>
        {
            error
        }
        Ok(_) => format!("{error}; additionally failed to prove clean detach"),
        Err(cleanup_error) => format!("{error}; additionally failed to detach: {cleanup_error}"),
    }
}

fn validate_options(options: &Options) -> Result<(), String> {
    if !(2..=30).contains(&options.stream_seconds) {
        return Err("live kernel stream seconds must be an integer from 2 to 30".to_owned());
    }
    if !(1..=200).contains(&options.poll_attempts) {
        return Err("live kernel polling attempts must be an integer from 1 to 200".to_owned());
    }
    if options.password.is_empty() || options.password.contains(['\r', '\n']) {
        return Err("CYANREX_ADMIN_PASSWORD must be a non-empty single-line value".to_owned());
    }
    if let Some(output) = options.report.as_deref() {
        match fs::symlink_metadata(output) {
            Ok(_) => {
                return Err(format!(
                    "live kernel evidence output already exists: {}",
                    output.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect live kernel evidence output {}: {error}",
                    output.display()
                ));
            }
        }
        if let Some(metadata) = options.release_metadata.as_deref() {
            evidence::candidate_from_metadata(metadata)?;
        }
    } else if options.release_metadata.is_some() {
        return Err("release metadata requires a live kernel evidence output".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "smoke_live_kernel_tests.rs"]
mod tests;
