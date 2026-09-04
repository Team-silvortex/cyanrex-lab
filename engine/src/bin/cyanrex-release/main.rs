mod arguments;
mod candidate;
mod candidate_cli;
mod evidence;
mod evidence_cli;
mod package;
mod package_cli;
mod release_archive;
mod release_metadata;
mod strict_json;

use std::env;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    match command {
        "--help" | "-h" | "help" => {
            print_help();
            Ok(())
        }
        "--version" | "-V" => {
            println!("cyanrex-release {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "candidate" => candidate_cli::run(&arguments[1..]),
        "evidence" => evidence_cli::run(&arguments[1..]),
        "package" => package_cli::run(&arguments[1..]),
        _ => Err(format!("unknown command {command:?}; run with --help")),
    }
}

fn print_help() {
    println!(
        "Cyanrex native release tooling\n\
         \n\
         Usage:\n\
           cyanrex-release candidate verify <bundle> [options]\n\
           cyanrex-release evidence <create|verify> [options]\n\
           cyanrex-release package extract <bundle> [options]\n\
           cyanrex-release --version\n\
         \n\
         Commands:\n\
           candidate   Verify a complete Tag candidate and optionally extract it\n\
           evidence    Create or strictly verify live-kernel acceptance evidence\n\
           package     Verify and safely extract an offline release package"
    );
}
