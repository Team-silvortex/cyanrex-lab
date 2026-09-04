use crate::arguments::ParsedArguments;
use crate::candidate;
use crate::release_metadata::Expectations;

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
        "verify" => run_verify(&arguments[1..]),
        command => Err(format!(
            "unknown candidate command {command:?}; run with candidate --help"
        )),
    }
}

fn run_verify(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--extract-to",
            "--expect-version",
            "--expect-revision",
            "--expect-tag",
            "--expect-source-state",
            "--expect-image-mode",
        ],
    )?;
    if parsed.positionals.len() != 1 {
        return Err("candidate verify requires exactly one bundle directory".to_owned());
    }
    let source_state = parsed.optional("--expect-source-state");
    if source_state
        .as_deref()
        .is_some_and(|value| !matches!(value, "clean" | "dirty" | "unavailable"))
    {
        return Err("expected source state must be clean, dirty, or unavailable".to_owned());
    }
    let image_mode = parsed.optional("--expect-image-mode");
    if image_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "built" | "prebuilt"))
    {
        return Err("expected image mode must be built or prebuilt".to_owned());
    }
    let expectations = Expectations {
        version: parsed.optional("--expect-version"),
        revision: parsed.optional("--expect-revision"),
        tag: parsed.optional("--expect-tag"),
        source_state,
        image_mode,
    };
    let verified = candidate::verify(
        parsed.positionals[0].as_ref(),
        &expectations,
        parsed.optional_path("--extract-to").as_deref(),
    )?;
    let revision = verified.metadata["source"]["revision"]
        .as_str()
        .unwrap_or("unavailable");
    println!(
        "[cyanrex] Release candidate verified: {} version={} revision={} evidence={}",
        verified.archive_name,
        verified.metadata["package"]["version"]
            .as_str()
            .unwrap_or("unknown"),
        revision,
        verified.report["generatedAt"].as_str().unwrap_or("unknown")
    );
    if let Some(path) = verified.extracted {
        println!("[cyanrex] Candidate safely extracted: {}", path.display());
    }
    Ok(())
}

fn print_help() {
    println!(
        "Strictly verify a complete Cyanrex release-candidate bundle\n\
         \n\
         Usage:\n\
           cyanrex-release candidate verify <bundle> [--extract-to <new-directory>]\n\
             [--expect-version <x.y.z>] [--expect-revision <sha>]\n\
             [--expect-tag <vx.y.z>] [--expect-source-state <state>]\n\
             [--expect-image-mode <mode>]"
    );
}
