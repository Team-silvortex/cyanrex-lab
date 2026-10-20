//! Public readiness only. Never reuse this LAN allowance for authenticated smoke operations.
use crate::arguments::ParsedArguments;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::Duration;

const MAX_BODY_BYTES: usize = 4096;

#[derive(Deserialize)]
struct Health {
    status: String,
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments == ["--help"] || arguments == ["-h"] {
        crate::smoke_cli::print_help();
        return Ok(());
    }
    let checks = arguments
        .iter()
        .filter(|value| value.as_str() == "--check-only")
        .count();
    if checks > 1 {
        return Err("--check-only may only be supplied once".into());
    }
    let arguments = arguments
        .iter()
        .filter(|value| value.as_str() != "--check-only")
        .cloned()
        .collect::<Vec<_>>();
    let parsed = ParsedArguments::new(
        &arguments,
        &["--engine-url", "--timeout-seconds", "--poll-seconds"],
    )?;
    if !parsed.positionals.is_empty() {
        return Err("wait-health does not accept positional arguments".into());
    }
    let mut url = health_url(parsed.required("--engine-url")?)?;
    let timeout = seconds(&parsed, "--timeout-seconds", 90, 3600)?;
    let poll = seconds(&parsed, "--poll-seconds", 2, 60)?;
    if checks == 1 {
        println!("[cyanrex] Health configuration valid; no request sent.");
        return Ok(());
    }
    url.set_path("/health");
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(timeout.min(Duration::from_secs(3)))
        .timeout(timeout)
        .build()
        .map_err(|_| "cannot initialize public health client")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "cannot initialize public health runtime")?;
    let result = runtime.block_on(async {
        let mut last_error = "no healthy response";
        tokio::time::timeout(timeout, async {
            loop {
                match probe(&client, &url).await {
                    Ok(()) => return,
                    Err(error) => last_error = error,
                }
                tokio::time::sleep(poll).await;
            }
        })
        .await
        .map_err(|_| {
            format!("Engine health deadline exceeded ({last_error}); readiness unconfirmed")
        })?;
        println!("[cyanrex] Engine health payload is ok; this is not full-stack readiness.");
        Ok(())
    });
    // A cancelled getaddrinfo task may be non-abortable. Do not wait for blocking resolver
    // threads after our deadline; this short-lived public-only CLI exits immediately afterward.
    runtime.shutdown_timeout(Duration::ZERO);
    result
}

fn seconds(
    parsed: &ParsedArguments,
    option: &str,
    default: u64,
    maximum: u64,
) -> Result<Duration, String> {
    let raw = parsed
        .optional(option)
        .unwrap_or_else(|| default.to_string());
    let value = if raw.bytes().all(|value| value.is_ascii_digit()) {
        raw.parse::<u64>().ok()
    } else {
        None
    };
    value
        .filter(|value| (1..=maximum).contains(value))
        .map(Duration::from_secs)
        .ok_or_else(|| format!("{option} must be an integer from 1 to {maximum}"))
}

fn health_url(raw: &str) -> Result<Url, String> {
    let error = "health URL must be an HTTP(S) origin without credentials, path, query or fragment";
    if raw.contains('@')
        || raw.chars().any(char::is_whitespace)
        || raw.chars().any(char::is_control)
    {
        return Err(error.into());
    }
    let url = Url::parse(raw).map_err(|_| error)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default().is_none_or(|port| port == 0)
    {
        return Err(error.into());
    }
    Ok(url)
}

async fn probe(client: &Client, url: &Url) -> Result<(), &'static str> {
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|_| "health request failed")?;
    if !response.status().is_success() {
        return Err("health HTTP status is not successful");
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BODY_BYTES as u64)
    {
        return Err("health response too large");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "health body read failed")?
    {
        if bytes
            .len()
            .checked_add(chunk.len())
            .is_none_or(|size| size > MAX_BODY_BYTES)
        {
            return Err("health response too large");
        }
        bytes.extend_from_slice(&chunk);
    }
    let health: Health =
        serde_json::from_slice(&bytes).map_err(|_| "health response is not valid typed JSON")?;
    if health.status != "ok" {
        return Err("health payload is not ok");
    }
    Ok(())
}
