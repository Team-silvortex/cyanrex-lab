//! Explicit one-query LAN diagnostic; never an Engine, service advertisement or pairing authority.
#[cfg(target_os = "linux")]
mod arguments;
#[cfg(target_os = "linux")]
mod output;

fn fail(code: i32, message: &str) -> ! {
    use std::io::Write;
    // A closed diagnostic pipe must not replace a fixed exit code with a formatting panic.
    let _ = writeln!(std::io::stderr().lock(), "{message}");
    std::process::exit(code);
}

#[cfg(target_os = "linux")]
fn browse(options: arguments::Options) -> Result<Vec<u8>, (i32, &'static str)> {
    use cyanrex_engine::{
        platform_host::prepare_platform_process, services::federation_mdns::discover_once,
    };
    let unsafe_process = (77, "discovery process check failed; no snapshot emitted");
    let checked = prepare_platform_process(&options.identity).map_err(|_| unsafe_process)?;
    checked
        .verify_before_runtime()
        .map_err(|_| unsafe_process)?;
    // No handler, worker or child is installed. Normal OS signal termination closes the process's
    // one owned socket; an interrupted CLI response must never be consumed as a successful snapshot.
    let peers = discover_once(
        &options.config,
        options.policy,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .map_err(|_| (74, "LAN discovery failed; no snapshot emitted"))?;
    checked
        .verify_current_thread()
        .map_err(|_| unsafe_process)?;
    let bytes = output::encode(&peers)
        .map_err(|_| (74, "discovery output rejected; no snapshot emitted"))?;
    checked
        .verify_current_thread()
        .map_err(|_| unsafe_process)?;
    Ok(bytes)
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        let mut args = std::env::args_os().skip(1);
        let first = args.next();
        if first.as_deref() == Some(std::ffi::OsStr::new("--help")) {
            if args.next().is_none() {
                if writeln!(std::io::stdout().lock(), "{}", arguments::USAGE).is_err() {
                    fail(74, "discovery output failed; snapshot not confirmed");
                }
                return;
            }
            fail(64, arguments::USAGE);
        }
        let options = match arguments::parse(first.into_iter().chain(args)) {
            Ok(options) => options,
            Err(()) => {
                fail(64, arguments::USAGE);
            }
        };
        let bytes = match browse(options) {
            Ok(bytes) => bytes,
            Err((code, message)) => {
                fail(code, message);
            }
        };
        // Bound bytes before the first write; stdout backpressure is not part of the network
        // deadline. A write error may leave a truncated stream, never a successful process result.
        if std::io::stdout().lock().write_all(&bytes).is_err() {
            fail(74, "discovery output failed; snapshot not confirmed");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        fail(69, "LAN discovery command requires Linux");
    }
}
