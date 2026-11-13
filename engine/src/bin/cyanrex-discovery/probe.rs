//! One explicit process-gated probe attempt; no controller, retained context or announcement sender.
use super::{probe_arguments, probe_output};
use cyanrex_engine::{
    platform_host::prepare_platform_process, services::federation_mdns::probe_once,
};
use std::{
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn run(options: probe_arguments::Options) -> Result<Vec<u8>, (i32, &'static str)> {
    let unsafe_process = (77, "probe process check failed; no observation emitted");
    let checked = prepare_platform_process(&options.identity).map_err(|_| unsafe_process)?;
    checked
        .verify_before_runtime()
        .map_err(|_| unsafe_process)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| (74, "probe clock unavailable; no observation emitted"))?
        .as_secs();
    let announcement = options
        .prepare_at(now)
        .map_err(|_| (64, "probe input rejected; no observation emitted"))?;
    let observation = probe_once(&options.config, &announcement, &AtomicBool::new(false))
        .map_err(|_| (74, "LAN probe failed; no observation emitted"))?;
    checked
        .verify_current_thread()
        .map_err(|_| unsafe_process)?;
    let bytes = probe_output::encode(observation)
        .map_err(|_| (74, "probe output rejected; no observation emitted"))?;
    checked
        .verify_current_thread()
        .map_err(|_| unsafe_process)?;
    Ok(bytes)
}
