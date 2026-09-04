use crate::evidence;
use crate::package::{self, VerifiedPackage};
use crate::release_archive::{
    archive_name_syntax, regular_file, verify_checksum_file, MAX_CONTROL_BYTES,
};
use crate::release_metadata::Expectations;
use crate::strict_json;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const EVIDENCE_NAME: &str = "cyanrex-live-kernel-acceptance.json";

pub struct VerifiedCandidate {
    pub metadata: Value,
    pub report: Value,
    pub archive_name: String,
    pub extracted: Option<PathBuf>,
}

pub fn verify(
    bundle: &Path,
    expectations: &Expectations,
    extract_to: Option<&Path>,
) -> Result<VerifiedCandidate, String> {
    let (archive, report_path) = discover_bundle(bundle)?;
    let package = package::verify_archive(&archive, expectations)?;
    let report = verify_evidence(&report_path, &package)?;
    let extracted = extract_to
        .map(|output| {
            package::extract_verified_archive(
                &package.archive,
                &package.package_root,
                &package.hashes,
                output,
            )
        })
        .transpose()?;
    let archive_name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .expect("discovered candidate archives have portable names")
        .to_owned();
    Ok(VerifiedCandidate {
        metadata: package.metadata,
        report,
        archive_name,
        extracted,
    })
}

fn discover_bundle(directory: &Path) -> Result<(PathBuf, PathBuf), String> {
    let root = fs::canonicalize(directory).map_err(|error| {
        format!(
            "cannot inspect candidate bundle {}: {error}",
            directory.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "candidate bundle is not a directory: {}",
            directory.display()
        ));
    }
    let entries = fs::read_dir(&root).map_err(|error| {
        format!(
            "cannot inspect candidate bundle {}: {error}",
            directory.display()
        )
    })?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot inspect candidate bundle {}: {error}",
                directory.display()
            )
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "candidate bundle contains a non-UTF-8 file name".to_owned())?;
        names.insert(name);
    }
    let archives = names
        .iter()
        .filter(|name| archive_name_syntax(name))
        .cloned()
        .collect::<Vec<_>>();
    if archives.len() != 1 {
        return Err("candidate bundle must contain exactly one Cyanrex .tar.gz archive".to_owned());
    }
    let archive_name = &archives[0];
    let expected = [
        archive_name.clone(),
        format!("{archive_name}.sha256"),
        EVIDENCE_NAME.to_owned(),
        format!("{EVIDENCE_NAME}.sha256"),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if names != expected {
        return Err(file_set_error(&expected, &names));
    }
    let archive = root.join(archive_name);
    let report = root.join(EVIDENCE_NAME);
    regular_file(&archive, "candidate archive", None)?;
    regular_file(&report, "live kernel evidence", Some(MAX_CONTROL_BYTES))?;
    verify_checksum_file(&root.join(format!("{archive_name}.sha256")), &archive)?;
    verify_checksum_file(&root.join(format!("{EVIDENCE_NAME}.sha256")), &report)?;
    Ok((archive, report))
}

fn verify_evidence(report_path: &Path, package: &VerifiedPackage) -> Result<Value, String> {
    let source = fs::read(report_path)
        .map_err(|error| format!("cannot read live kernel evidence: {error}"))?;
    let report = strict_json::from_slice(&source)
        .map_err(|error| format!("cannot parse live kernel evidence: {error}"))?;
    evidence::validate_report(&report)
        .map_err(|error| format!("live kernel evidence is invalid: {error}"))?;
    let expected = candidate_binding(&package.metadata, &package.metadata_source);
    if report["candidate"] != expected {
        return Err("live kernel evidence does not match the archived release metadata".to_owned());
    }
    Ok(report)
}

fn candidate_binding(metadata: &Value, metadata_source: &[u8]) -> Value {
    let images = &metadata["images"];
    json!({
        "releaseMetadataSha256": sha256(metadata_source),
        "package": metadata["package"].clone(),
        "source": metadata["source"].clone(),
        "images": {
            "mode": images["mode"].clone(),
            "references": images["references"].clone(),
            "contentIds": images["contentIds"].clone(),
            "archiveSha256": images["archive"]["sha256"].clone(),
        },
    })
}

fn sha256(source: &[u8]) -> String {
    Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn file_set_error(expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> String {
    let mut details = Vec::new();
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(expected).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        details.push(format!("missing {}", missing.join(", ")));
    }
    if !unexpected.is_empty() {
        details.push(format!("unexpected {}", unexpected.join(", ")));
    }
    format!(
        "candidate bundle file set is invalid: {}",
        details.join("; ")
    )
}
