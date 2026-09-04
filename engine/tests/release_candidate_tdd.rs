use std::path::Path;
use std::process::Command;

#[test]
fn native_candidate_verifier_passes_the_complete_bundle_regression_suite() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Engine crate is inside the repository")
        .join("scripts/test-release-candidate.sh");
    let output = Command::new("bash")
        .arg(script)
        .env(
            "CYANREX_NATIVE_RELEASE_TOOL",
            env!("CARGO_BIN_EXE_cyanrex-release"),
        )
        .output()
        .expect("candidate regression script should run");
    assert!(
        output.status.success(),
        "native candidate regression failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
