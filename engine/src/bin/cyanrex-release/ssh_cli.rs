use std::{fs, path::Path, process::Stdio, time::Duration};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::arguments::ParsedArguments;

pub fn run(arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() || arguments.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("SSH management of a pre-installed, verified offline package\n\
            Usage: cyanrex-release ssh <plan|apply> --host <SSH-config-alias>\n\
              --directory </absolute/path/cyanrex-lab-VERSION-TIMESTAMP> --known-hosts </trusted/known_hosts>\n\
              --action <up|down|status> [--confirm <exact-plan-confirmation>]\n\
            plan is offline. apply requires the plan confirmation, including for status.\n\
            Keys stay in OpenSSH/ssh-agent; host keys must already be independently verified.\n\
            No upload, host provisioning, automatic upgrades, port exposure or volume deletion.");
        return Ok(());
    }
    let mode = arguments[0].as_str();
    if !matches!(mode, "plan" | "apply") {
        return Err("SSH mode must be plan or apply".into());
    }
    let parsed = ParsedArguments::new(
        &arguments[1..],
        &[
            "--host",
            "--directory",
            "--known-hosts",
            "--action",
            "--confirm",
        ],
    )?;
    if !parsed.positionals.is_empty() {
        return Err("unexpected SSH positional arguments".into());
    }
    let host = parsed.required("--host")?;
    if host.is_empty()
        || host.len() > 253
        || !host.as_bytes()[0].is_ascii_alphanumeric()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("SSH host must be a literal hostname or trusted SSH-config alias, not a command, URI or user@host".into());
    }
    let directory = parsed.required("--directory")?;
    safe_path(directory)?;
    let version = env!("CARGO_PKG_VERSION");
    let package_name = Path::new(directory)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("missing remote package directory name")?;
    let package = crate::release_archive::parse_archive_name(&format!("{package_name}.tar.gz"))?;
    if package.version != version {
        return Err(format!(
            "remote package must be version {version}; no implicit upgrade or downgrade"
        ));
    }
    let action = parsed.required("--action")?;
    check_action(action)?;
    let known_hosts = parsed.required("--known-hosts")?;
    safe_path(known_hosts)?;
    let metadata =
        fs::symlink_metadata(known_hosts).map_err(|_| "trusted known_hosts file is unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() > 4 * 1024 * 1024 {
        return Err("known_hosts must be a regular file no larger than 4 MiB".into());
    }
    let known_hosts_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(known_hosts).map_err(|_| "cannot read trusted known_hosts")?)
    );
    let remote = format!("cd '{directory}' && exec ./cyanrex-release ssh-target --expect-version '{version}' --action '{action}'");
    let mut args = vec!["-T".to_string(), "-n".into()];
    for option in [
        "StrictHostKeyChecking=yes",
        "BatchMode=yes",
        "PasswordAuthentication=no",
        "KbdInteractiveAuthentication=no",
        "PreferredAuthentications=publickey",
        "ForwardAgent=no",
        "ForwardX11=no",
        "ClearAllForwardings=yes",
        "PermitLocalCommand=no",
        "ControlMaster=no",
        "ControlPath=none",
        "UpdateHostKeys=no",
        "KnownHostsCommand=none",
        "VerifyHostKeyDNS=no",
        "ForkAfterAuthentication=no",
        "SessionType=default",
        "GlobalKnownHostsFile=/dev/null",
        "ConnectTimeout=10",
        "ConnectionAttempts=1",
        "ServerAliveInterval=15",
        "ServerAliveCountMax=2",
    ] {
        args.extend(["-o".into(), option.into()]);
    }
    args.extend([
        "-o".into(),
        format!("UserKnownHostsFile={known_hosts}"),
        host.into(),
        remote,
    ]);
    let mut plan = json!({"transport": "ssh", "host": host, "directory": directory, "action": action,
        "expected_version": version, "known_hosts_sha256": known_hosts_hash,
        "changes_remote_state": action != "status", "ssh_arguments": args});
    let confirmation = format!("APPLY {:x}", Sha256::digest(plan.to_string().as_bytes()));
    plan["confirmation"] = json!(confirmation);
    if mode == "plan" {
        if parsed.optional("--confirm").is_some() {
            return Err("--confirm belongs to ssh apply, not plan".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if parsed.required("--confirm")? != confirmation {
        return Err("confirmation does not match this host, package, action and known_hosts; review a fresh plan".into());
    }
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|error| error.to_string())?.block_on(async {
        // Use the OS client, never a browser or Engine shell endpoint. Local SSH configuration
        // (including ProxyJump/ProxyCommand) is trusted operator input, not discovered metadata.
        let mut command = tokio::process::Command::new("ssh");
        command.args(&args).stdin(Stdio::null()).kill_on_drop(true);
        let status = tokio::time::timeout(Duration::from_secs(600), command.status()).await
            .map_err(|_| "SSH deadline exceeded; remote state is uncertain, inspect status before retrying")?
            .map_err(|_| "cannot execute the local OpenSSH client")?;
        if !status.success() { return Err("SSH failed; remote changes may have started. Inspect status before retrying; no rollback was attempted".into()); }
        Ok(())
    })
}

pub fn run_target(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(arguments, &["--expect-version", "--action"])?;
    if !parsed.positionals.is_empty() {
        return Err("unexpected SSH target arguments".into());
    }
    let expected = parsed.required("--expect-version")?;
    if expected != env!("CARGO_PKG_VERSION") {
        return Err("SSH deployment version mismatch; no automatic upgrade or downgrade".into());
    }
    let action = parsed.required("--action")?;
    check_action(action)?;
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    crate::installed_package::verify_deployment_controls(&directory, expected)?;
    // .env is private, pre-provisioned operator configuration. Do not transfer it, print it,
    // regenerate secrets, acquire sudo, or add --volumes/--remove-orphans here.
    let env = fs::symlink_metadata(directory.join(".env"))
        .map_err(|_| "prepare a private .env on the target before SSH deployment")?;
    if !env.file_type().is_file() {
        return Err("target .env must be a regular private file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if env.permissions().mode() & 0o077 != 0 || env.uid() != unsafe { libc::geteuid() } {
            return Err(
                "target .env must be owned by the SSH user and inaccessible to group/others".into(),
            );
        }
    }
    let status = std::process::Command::new(directory.join("deploy.sh"))
        .arg(action)
        .stdin(Stdio::null())
        .status()
        .map_err(|_| "cannot execute verified deploy.sh")?;
    if !status.success() {
        return Err("target deployment command failed; inspect state before retrying".into());
    }
    Ok(())
}

fn check_action(action: &str) -> Result<(), String> {
    if !matches!(action, "up" | "down" | "status") {
        return Err("SSH action must be up, down or status".into());
    }
    Ok(())
}

fn safe_path(value: &str) -> Result<(), String> {
    if !value.starts_with('/')
        || value.len() > 1024
        || value
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err("SSH paths must be absolute, with literal ASCII letters/digits, slash, dot, underscore or hyphen; no traversal or shell syntax".into());
    }
    Ok(())
}
