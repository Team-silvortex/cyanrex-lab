pub(super) use super::{
    fixture::{fifo, Fixture, Run, Watch, CANARY},
    legacy::{address, input, listener, no_dial, no_host, root, ChildGuard},
};
use std::{ffi::OsString, net::SocketAddr, path::Path};
pub(super) fn document(root: &Path, port: u16, bind: SocketAddr) -> serde_json::Value {
    let mut value = super::legacy::document(root, port, bind);
    value["origin"] = format!("https://{bind}").into();
    value
}
pub(super) fn arguments(
    f: &Fixture,
    input: &Path,
    bind: SocketAddr,
    cert: &Path,
    key: &Path,
) -> Vec<OsString> {
    let mut args = super::legacy::arguments(f, input, bind);
    args[0] = "serve-loopback-tls".into();
    args.extend([
        "--cert".into(),
        cert.as_os_str().to_owned(),
        "--key".into(),
        key.as_os_str().to_owned(),
    ]);
    args
}
pub(super) fn run(f: &Fixture, args: &[OsString]) -> Run {
    super::legacy::run(f, args, None)
}
pub(super) fn rejected(run: &Run, code: i32) {
    super::legacy::rejected(run, code);
    if code == 1 {
        assert_eq!(run.output.stderr, b"platform loopback host failed\n");
    }
}
