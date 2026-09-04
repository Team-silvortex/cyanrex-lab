use crate::arguments::ParsedArguments;
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
        command => Err(format!(
            "unknown package command {command:?}; run with package --help"
        )),
    }
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
        "Verify and safely extract a Cyanrex offline release package\n\
         \n\
         Usage:\n\
           cyanrex-release package extract <bundle> --output <new-directory>\n\
             [--expect-version <x.y.z>] [--expect-revision <sha>]\n\
             [--expect-tag <vx.y.z>] [--expect-source-state <state>]\n\
             [--expect-image-mode <mode>]"
    );
}
