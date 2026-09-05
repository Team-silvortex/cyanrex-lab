use crate::arguments::ParsedArguments;
use crate::smoke_health;
use crate::smoke_live_kernel::{self, Options as LiveKernelOptions};
use crate::smoke_runner_agent::{self, Options as RunnerAgentOptions};
use std::env;
use std::path::PathBuf;
use std::time::Duration;

pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|value| value == "--help" || value == "-h")
    {
        print_help();
        return Ok(());
    }
    match arguments[0].as_str() {
        "health" => run_health(&arguments[1..]),
        "live-kernel" => run_live_kernel(&arguments[1..]),
        "runner-agent" => run_runner_agent(&arguments[1..]),
        command => Err(format!(
            "unknown smoke command {command:?}; run with smoke --help"
        )),
    }
}

fn run_health(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(arguments, &["--engine-url"])?;
    if !parsed.positionals.is_empty() {
        return Err("smoke health does not accept positional arguments".to_owned());
    }
    let engine_url = option_or_env(
        &parsed,
        "--engine-url",
        "CYANREX_SMOKE_ENGINE_URL",
        "http://127.0.0.1:8080",
    );
    runtime()?.block_on(smoke_health::run(&engine_url))?;
    println!("[cyanrex] Engine health payload is ok.");
    Ok(())
}

fn run_runner_agent(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--engine-url",
            "--origin",
            "--agent-id",
            "--ready-attempts",
            "--ready-interval-ms",
            "--job-attempts",
            "--job-interval-ms",
        ],
    )?;
    if !parsed.positionals.is_empty() {
        return Err("smoke runner-agent does not accept positional arguments".to_owned());
    }
    let options = RunnerAgentOptions {
        engine_url: option_or_env(
            &parsed,
            "--engine-url",
            "CYANREX_SMOKE_ENGINE_URL",
            "http://127.0.0.1:8080",
        ),
        origin: option_or_env(
            &parsed,
            "--origin",
            "CYANREX_SMOKE_ORIGIN",
            "http://localhost:3000",
        ),
        agent_id: option_or_env(
            &parsed,
            "--agent-id",
            "CYANREX_AGENT_ID",
            "cyanrex-docker-compiler",
        ),
        password: required_env("CYANREX_ADMIN_PASSWORD")?,
        totp_secret: required_env("CYANREX_ADMIN_TOTP_SECRET")?,
        ready_attempts: numeric_option(
            &parsed,
            "--ready-attempts",
            "CYANREX_AGENT_SMOKE_READY_ATTEMPTS",
            30,
            1,
            300,
        )? as u32,
        ready_interval: Duration::from_millis(numeric_option(
            &parsed,
            "--ready-interval-ms",
            "CYANREX_AGENT_SMOKE_READY_INTERVAL_MS",
            1_000,
            0,
            60_000,
        )?),
        job_attempts: numeric_option(
            &parsed,
            "--job-attempts",
            "CYANREX_AGENT_SMOKE_JOB_ATTEMPTS",
            70,
            1,
            600,
        )? as u32,
        job_interval: Duration::from_millis(numeric_option(
            &parsed,
            "--job-interval-ms",
            "CYANREX_AGENT_SMOKE_JOB_INTERVAL_MS",
            500,
            0,
            60_000,
        )?),
    };
    let runtime = runtime()?;
    let job_id = runtime.block_on(smoke_runner_agent::run(&options))?;
    println!("[cyanrex] Remote compiler smoke test passed: {job_id}");
    Ok(())
}

fn run_live_kernel(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--engine-url",
            "--origin",
            "--stream-seconds",
            "--poll-attempts",
            "--poll-interval-ms",
            "--report",
            "--release-metadata",
        ],
    )?;
    if !parsed.positionals.is_empty() {
        return Err("smoke live-kernel does not accept positional arguments".to_owned());
    }
    let options = LiveKernelOptions {
        engine_url: option_or_env(
            &parsed,
            "--engine-url",
            "CYANREX_SMOKE_ENGINE_URL",
            "http://127.0.0.1:8080",
        ),
        origin: option_or_env(
            &parsed,
            "--origin",
            "CYANREX_SMOKE_ORIGIN",
            "http://localhost:3000",
        ),
        password: required_env("CYANREX_ADMIN_PASSWORD")?,
        totp_secret: required_env("CYANREX_ADMIN_TOTP_SECRET")?,
        stream_seconds: numeric_option(
            &parsed,
            "--stream-seconds",
            "CYANREX_KERNEL_SMOKE_STREAM_SECONDS",
            6,
            2,
            30,
        )? as u32,
        poll_attempts: numeric_option(
            &parsed,
            "--poll-attempts",
            "CYANREX_KERNEL_SMOKE_POLL_ATTEMPTS",
            80,
            1,
            200,
        )? as u32,
        poll_interval: live_kernel_poll_interval(&parsed)?,
        report: path_option_or_env(&parsed, "--report", "CYANREX_KERNEL_SMOKE_REPORT"),
        release_metadata: parsed.optional_path("--release-metadata"),
    };
    let outcome = runtime()?.block_on(smoke_live_kernel::run(&options))?;
    if let Some(report) = outcome.report {
        println!(
            "[cyanrex] Live kernel acceptance evidence: {}",
            report.display()
        );
    }
    println!(
        "[cyanrex] Live kernel attach, event stream, and detach acceptance passed: {} -> {}",
        outcome.program_name, outcome.pin_path
    );
    Ok(())
}

fn option_or_env(parsed: &ParsedArguments, option: &str, variable: &str, default: &str) -> String {
    parsed
        .optional(option)
        .or_else(|| env::var(variable).ok())
        .unwrap_or_else(|| default.to_owned())
}

fn required_env(name: &str) -> Result<String, String> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn path_option_or_env(parsed: &ParsedArguments, option: &str, variable: &str) -> Option<PathBuf> {
    parsed.optional_path(option).or_else(|| {
        env::var(variable)
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    })
}

fn live_kernel_poll_interval(parsed: &ParsedArguments) -> Result<Duration, String> {
    if parsed.optional("--poll-interval-ms").is_some()
        || env::var("CYANREX_KERNEL_SMOKE_POLL_INTERVAL_MS").is_ok()
    {
        return numeric_option(
            parsed,
            "--poll-interval-ms",
            "CYANREX_KERNEL_SMOKE_POLL_INTERVAL_MS",
            250,
            0,
            60_000,
        )
        .map(Duration::from_millis);
    }
    let source =
        env::var("CYANREX_KERNEL_SMOKE_POLL_INTERVAL").unwrap_or_else(|_| "0.25".to_owned());
    let seconds = source.parse::<f64>().map_err(|_| {
        "CYANREX_KERNEL_SMOKE_POLL_INTERVAL must be a number from 0 to 60".to_owned()
    })?;
    if !seconds.is_finite() || !(0.0..=60.0).contains(&seconds) {
        return Err("CYANREX_KERNEL_SMOKE_POLL_INTERVAL must be a number from 0 to 60".to_owned());
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot initialize smoke runtime: {error}"))
}

fn numeric_option(
    parsed: &ParsedArguments,
    option: &str,
    variable: &str,
    default: u64,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let source = parsed.optional(option).or_else(|| env::var(variable).ok());
    let Some(source) = source else {
        return Ok(default);
    };
    let value = source
        .parse::<u64>()
        .map_err(|_| format!("{option} must be an integer from {minimum} to {maximum}"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!(
            "{option} must be an integer from {minimum} to {maximum}"
        ));
    }
    Ok(value)
}

fn print_help() {
    println!(
        "Run native release acceptance probes\n\
         \n\
         Usage:\n\
           cyanrex-release smoke health [options]\n\
           cyanrex-release smoke live-kernel [options]\n\
           cyanrex-release smoke runner-agent [options]\n\
         \n\
         Health options:\n\
           --engine-url <origin>       Engine URL (default: CYANREX_SMOKE_ENGINE_URL)\n\
         \n\
         Live kernel options:\n\
           --engine-url <origin>       Engine URL (default: CYANREX_SMOKE_ENGINE_URL)\n\
           --origin <origin>           Trusted frontend Origin\n\
           --stream-seconds <2-30>     Kernel event sampling window\n\
           --poll-attempts <1-200>     Event-history polling limit\n\
           --poll-interval-ms <ms>     Delay between event-history polls\n\
           --report <path>             Optional atomic evidence output\n\
           --release-metadata <path>   Bind evidence to packaged release metadata\n\
         \n\
         Runner Agent options:\n\
           --engine-url <origin>       Engine URL (default: CYANREX_SMOKE_ENGINE_URL)\n\
           --origin <origin>           Trusted frontend Origin\n\
           --agent-id <id>             Compile Agent to exercise\n\
         \n\
         CYANREX_ADMIN_PASSWORD and CYANREX_ADMIN_TOTP_SECRET are read only from the environment."
    );
}
