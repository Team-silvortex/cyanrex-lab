//! Explicit local C1 provisioning, never live Engine startup, migration or public enrollment.
#[cfg(unix)]
mod arguments;
#[cfg(unix)]
mod config;
#[cfg(unix)]
mod database;
#[cfg(unix)]
mod enrollment;
#[cfg(unix)]
mod private_file;

#[cfg(unix)]
type Result<T> = std::result::Result<T, &'static str>;

fn main() {
    // Do this before constructing Tokio (and before any worker thread). PgConnectOptions reads
    // PG* even with new_without_pgpass(); neither ambient configuration nor ~/.pgpass is authority.
    for name in std::env::vars_os()
        .map(|(name, _)| name)
        .collect::<Vec<_>>()
    {
        if name.as_encoded_bytes().starts_with(b"PG") {
            std::env::remove_var(name);
        }
    }
    #[cfg(unix)]
    {
        let outcome = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "runtime_unavailable")
            .and_then(|runtime| runtime.block_on(run()));
        if let Err(code) = outcome {
            // Never include raw arguments, paths, config, SQL errors or secret-bearing results.
            let action = match code {
                "bootstrap_unconfirmed" => "preserve the enrollment marker; inspect the pinned target and reconcile with a trusted operator; do not automatically retry",
                "committed_delivery_unconfirmed" => "database commit was confirmed; preserve private output and reconcile secret delivery; do not reinitialize",
                code if code.starts_with("reconciliation_") => "read-only reconciliation did not confirm a consistent snapshot; no repair or retry is authorized; preserve data and investigate the reported boundary",
                _ => "no bootstrap was dispatched; correct the explicit inputs and review a new plan",
            };
            eprintln!("{}", serde_json::json!({"error":code,"next_action":action}));
            std::process::exit(if code == "committed_delivery_unconfirmed" {
                3
            } else if code == "bootstrap_unconfirmed" {
                4
            } else {
                2
            });
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("cyanrex-provision requires Unix private-file semantics; use Linux or WSL2");
        std::process::exit(2);
    }
}

#[cfg(unix)]
async fn run() -> Result<()> {
    use cyanrex_engine::services::auth_service::durable_source::DurableAuthSource;
    let arguments = arguments::parse(std::env::args_os().skip(1).collect())?;
    let Some(args) = arguments else {
        return Ok(());
    };
    let config = config::Config::read(&args.config)?;
    let password = args
        .password
        .as_ref()
        .map(|path| config::read_password(path))
        .transpose()?;
    let database = database::Database::connect(&config).await?;
    let snapshot = database.snapshot(&config).await?;
    if args.verb == "reconcile" {
        database.pin(snapshot.confirmation(&config))?;
        let source = DurableAuthSource::new(database.pool.clone(), config.authority_id);
        let report = source
            .reconcile_authority(config.scope())
            .await
            .map_err(|error| error.code())?;
        let mut target = config.public_target();
        // The initial provisioning username is not a lifecycle filter or required survivor.
        if let Some(fields) = target.as_object_mut() {
            fields.remove("username");
        }
        return print_report(
            &serde_json::json!({"format_version":1,"status":"consistent_snapshot",
            "target":target,"reconciliation":report,"live_runtime_changed":false,
            "observation":"bounded read-time consistency, not current authorization, a bootstrap receipt, secret delivery or permission to retry"}),
            "console_unavailable",
        );
    }
    if args.verb == "inspect" {
        return print_report(&snapshot.report(&config, false), "console_unavailable");
    }
    if !snapshot.empty() {
        return Err("namespace_not_empty");
    }
    if !snapshot.can_create {
        return Err("target_unavailable");
    }
    if args.verb == "plan" {
        return print_report(&snapshot.report(&config, true), "console_unavailable");
    }
    let confirmation = snapshot.confirmation(&config);
    if args.confirmation.as_deref() != Some(&confirmation) {
        return Err("confirmation_mismatch");
    }
    database.pin(confirmation.clone())?;
    // Reserve/sync a non-secret marker before mutation. Never overwrite an earlier run's output.
    let mut output = enrollment::Enrollment::reserve(
        args.enrollment.as_ref().ok_or("invalid_arguments")?,
        &confirmation,
    )?;
    let source = DurableAuthSource::new(database.pool.clone(), config.authority_id);
    let result = source
        .bootstrap_empty_authority(
            config.scope(),
            &config.username,
            password.as_deref().ok_or("invalid_arguments")?,
        )
        .await
        .map_err(|_| "bootstrap_unconfirmed")?;
    output
        .publish(&config, &confirmation, &result)
        .map_err(|_| "committed_delivery_unconfirmed")?;
    let report = serde_json::json!({"format_version":1,"status":"committed_enrollment_saved",
        "confirmation":confirmation,"target":config.public_target(),
        "account_id":result.registration.account.account_id,
        "principal_id":result.identity.principal.reference.principal_id,
        "session_issued":false,"live_runtime_changed":false});
    print_report(&report, "committed_delivery_unconfirmed")
}

#[cfg(unix)]
fn print_report(value: &serde_json::Value, error: &'static str) -> Result<()> {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|_| error)?;
    stdout
        .write_all(b"\n")
        .and_then(|_| stdout.flush())
        .map_err(|_| error)
}
