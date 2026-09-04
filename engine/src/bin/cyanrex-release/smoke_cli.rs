use crate::arguments::ParsedArguments;
use crate::smoke_runner_agent::{self, Options};
use std::env;
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
        "runner-agent" => run_runner_agent(&arguments[1..]),
        command => Err(format!(
            "unknown smoke command {command:?}; run with smoke --help"
        )),
    }
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
    let options = Options {
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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot initialize smoke runtime: {error}"))?;
    let job_id = runtime.block_on(smoke_runner_agent::run(&options))?;
    println!("[cyanrex] Remote compiler smoke test passed: {job_id}");
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
           cyanrex-release smoke runner-agent [options]\n\
         \n\
         Runner Agent options:\n\
           --engine-url <origin>       Engine URL (default: CYANREX_SMOKE_ENGINE_URL)\n\
           --origin <origin>           Trusted frontend Origin\n\
           --agent-id <id>             Compile Agent to exercise\n\
         \n\
         CYANREX_ADMIN_PASSWORD and CYANREX_ADMIN_TOTP_SECRET are read only from the environment."
    );
}
