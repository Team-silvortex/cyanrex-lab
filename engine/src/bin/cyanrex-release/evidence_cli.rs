use crate::evidence::{self, CreateOptions, VerifyOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
        "create" => run_create(&arguments[1..]),
        "verify" => run_verify(&arguments[1..]),
        command => Err(format!(
            "unknown evidence command {command:?}; run with evidence --help"
        )),
    }
}

fn run_create(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--output",
            "--environment",
            "--event",
            "--program-name",
            "--pin-path",
            "--release-metadata",
        ],
    )?;
    if !parsed.positionals.is_empty() {
        return Err("evidence create does not accept positional arguments".to_owned());
    }
    let options = CreateOptions {
        output: parsed.required_path("--output")?,
        environment: parsed.required_path("--environment")?,
        event: parsed.required_path("--event")?,
        program_name: parsed.required("--program-name")?.to_owned(),
        pin_path: parsed.required("--pin-path")?.to_owned(),
        release_metadata: parsed.optional_path("--release-metadata"),
    };
    let output = evidence::create(&options)?;
    println!(
        "[cyanrex] Live kernel acceptance evidence: {}",
        output.display()
    );
    Ok(())
}

fn run_verify(arguments: &[String]) -> Result<(), String> {
    let parsed = ParsedArguments::new(
        arguments,
        &[
            "--release-metadata",
            "--expect-version",
            "--expect-revision",
            "--expect-tag",
            "--expect-source-state",
            "--expect-image-mode",
        ],
    )?;
    if parsed.positionals.len() != 1 {
        return Err("evidence verify requires exactly one report path".to_owned());
    }
    let options = VerifyOptions {
        report: PathBuf::from(&parsed.positionals[0]),
        release_metadata: parsed.optional_path("--release-metadata"),
        expect_version: parsed.optional("--expect-version"),
        expect_revision: parsed.optional("--expect-revision"),
        expect_tag: parsed.optional("--expect-tag"),
        expect_source_state: parsed.optional("--expect-source-state"),
        expect_image_mode: parsed.optional("--expect-image-mode"),
    };
    evidence::verify(&options)?;
    println!(
        "[cyanrex] Live kernel acceptance evidence verified: {}",
        options.report.display()
    );
    Ok(())
}

fn print_help() {
    println!(
        "Create or strictly verify live-kernel acceptance evidence\n\
         \n\
         Usage:\n\
           cyanrex-release evidence create --output <path> --environment <path>\n\
             --event <path> --program-name <name> --pin-path <path>\n\
             [--release-metadata <path>]\n\
           cyanrex-release evidence verify <report> [--release-metadata <path>]\n\
             [--expect-version <x.y.z>] [--expect-revision <sha>]\n\
             [--expect-tag <vx.y.z>] [--expect-source-state <state>]\n\
             [--expect-image-mode <mode>]"
    );
}

struct ParsedArguments {
    positionals: Vec<String>,
    options: BTreeMap<String, String>,
}

impl ParsedArguments {
    fn new(arguments: &[String], allowed: &[&str]) -> Result<Self, String> {
        let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
        let mut positionals = Vec::new();
        let mut options = BTreeMap::new();
        let mut index = 0;
        while index < arguments.len() {
            let argument = &arguments[index];
            if !argument.starts_with("--") {
                positionals.push(argument.clone());
                index += 1;
                continue;
            }
            if !allowed.contains(argument.as_str()) {
                return Err(format!("unknown option {argument:?}"));
            }
            let value = arguments
                .get(index + 1)
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| format!("{argument} requires a value"))?;
            if options.insert(argument.clone(), value.clone()).is_some() {
                return Err(format!("option {argument} may only be supplied once"));
            }
            index += 2;
        }
        Ok(Self {
            positionals,
            options,
        })
    }

    fn required(&self, name: &str) -> Result<&str, String> {
        self.options
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("missing required option {name}"))
    }

    fn required_path(&self, name: &str) -> Result<PathBuf, String> {
        self.required(name).map(PathBuf::from)
    }

    fn optional(&self, name: &str) -> Option<String> {
        self.options.get(name).cloned()
    }

    fn optional_path(&self, name: &str) -> Option<PathBuf> {
        self.options.get(name).map(PathBuf::from)
    }
}
