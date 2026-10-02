use super::Result;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

pub struct Arguments {
    pub verb: String,
    pub config: PathBuf,
    pub password: Option<PathBuf>,
    pub enrollment: Option<PathBuf>,
    pub confirmation: Option<String>,
}
pub fn parse(values: Vec<OsString>) -> Result<Option<Arguments>> {
    if values.len() == 1 && values[0] == "--version" {
        println!("cyanrex-provision {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    if values.len() == 1 && (values[0] == "--help" || values[0] == "-h") {
        println!("Cyanrex explicit empty-authority provisioning (Unix, internal staging)\n\
            Usage:\n\
              cyanrex-provision plan --config /absolute/private/target.json\n\
              cyanrex-provision inspect --config /absolute/private/target.json\n\
              cyanrex-provision reconcile --config /absolute/private/target.json\n\
              cyanrex-provision apply --config /absolute/private/target.json --confirm <reviewed-plan-digest> --password-file /absolute/private/password --enrollment-file /absolute/private/NEW-enrollment.json\n\
            No environment defaults, public setup, migrations, automatic retries or login.\n\
            Configuration/password files must be owned private regular files (0600 or 0400).\n\
            The output parent must be owned 0700; existing output is never overwritten.\n\
            Local Unix sockets or explicit 127.0.0.1/::1 only; no DNS or remote network targets.\n\
            Inspect reports catalog occupancy, not health, a commit receipt or secret delivery.\n\
            Reconcile checks a bounded read-only legacy snapshot; it never repairs or restores secrets.\n\
            See docs/en/collaboration-provisioning.md before using this staging tool.");
        return Ok(None);
    }
    let verb = values
        .first()
        .and_then(|s| s.to_str())
        .ok_or("invalid_arguments")?;
    if !["plan", "inspect", "apply", "reconcile"].contains(&verb) || values.len() % 2 != 1 {
        return Err("invalid_arguments");
    }
    let allowed = if verb == "apply" {
        &[
            "--config",
            "--confirm",
            "--password-file",
            "--enrollment-file",
        ][..]
    } else {
        &["--config"][..]
    };
    let mut options = BTreeMap::new();
    for pair in values[1..].chunks_exact(2) {
        let key = pair[0].to_str().ok_or("invalid_arguments")?;
        if !allowed.contains(&key) || options.insert(key, pair[1].clone()).is_some() {
            return Err("invalid_arguments");
        }
    }
    if options.len() != allowed.len() {
        return Err("invalid_arguments");
    }
    let confirmation = options
        .get("--confirm")
        .map(|s| s.to_str().ok_or("invalid_arguments").map(str::to_owned))
        .transpose()?;
    if confirmation.as_ref().is_some_and(|s| {
        s.len() != 64
            || !s
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err("invalid_arguments");
    }
    Ok(Some(Arguments {
        verb: verb.to_owned(),
        config: PathBuf::from(&options["--config"]),
        password: options.get("--password-file").map(PathBuf::from),
        enrollment: options.get("--enrollment-file").map(PathBuf::from),
        confirmation,
    }))
}
