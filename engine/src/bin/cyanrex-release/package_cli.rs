use crate::arguments::ParsedArguments;
use crate::installed_package;
use crate::package;
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
        "extract" => run_extract(&arguments[1..]),
        "verify" => run_verify(&arguments[1..]),
        "verify-loaded-images" => run_verify_loaded_images(&arguments[1..]),
        command => Err(format!(
            "unknown package command {command:?}; run with package --help"
        )),
    }
}

fn exactly_one_path(arguments: &[String], command: &str) -> Result<std::path::PathBuf, String> {
    let parsed = ParsedArguments::new(arguments, &[])?;
    if parsed.positionals.len() != 1 {
        return Err(format!(
            "package {command} requires exactly one extracted package directory"
        ));
    }
    Ok(std::path::PathBuf::from(&parsed.positionals[0]))
}

fn run_verify(arguments: &[String]) -> Result<(), String> {
    let package = exactly_one_path(arguments, "verify")?;
    let metadata = installed_package::verify(&package)?;
    println!(
        "[cyanrex] Extracted release package verified: {} ({})",
        metadata["package"]["name"].as_str().unwrap_or("unknown"),
        metadata["package"]["version"].as_str().unwrap_or("unknown")
    );
    Ok(())
}

fn run_verify_loaded_images(arguments: &[String]) -> Result<(), String> {
    let package = exactly_one_path(arguments, "verify-loaded-images")?;
    installed_package::verify_loaded_images(&package)?;
    println!("[cyanrex] Loaded Docker image identities match the release package.");
    Ok(())
}

fn run_extract(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--output",
            "--expect-version",
            "--expect-revision",
            "--expect-tag",
            "--expect-source-state",
            "--expect-image-mode",
        ],
    )?;
    if parsed.positionals.len() != 1 {
        return Err("package extract requires exactly one bundle directory".to_owned());
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
    let bundle = &parsed.positionals[0];
    let output = parsed.required_path("--output")?;
    let extracted = package::verify_and_extract(bundle.as_ref(), &output, &expectations)?;
    println!(
        "[cyanrex] Release package verified and safely extracted: {} -> {}",
        extracted.metadata["package"]["version"]
            .as_str()
            .unwrap_or("unknown"),
        extracted.path.display()
    );
    Ok(())
}

fn print_help() {
    println!(
        "Verify, inspect, and safely extract a Cyanrex offline release package\n\
         \n\
         Usage:\n\
           cyanrex-release package extract <bundle> --output <new-directory>\n\
             [--expect-version <x.y.z>] [--expect-revision <sha>]\n\
             [--expect-tag <vx.y.z>] [--expect-source-state <state>]\n\
             [--expect-image-mode <mode>]\n\
           cyanrex-release package verify <extracted-package>\n\
           cyanrex-release package verify-loaded-images <extracted-package>\n\
         \n\
         verify checks a freshly extracted directory before runtime files are created.\n\
         verify-loaded-images checks checksum-bound metadata and loaded Docker IDs only."
    );
}
