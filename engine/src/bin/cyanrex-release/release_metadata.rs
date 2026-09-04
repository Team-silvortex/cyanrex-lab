use crate::release_archive::{decimal_triplet, lowercase_hex, parse_archive_name, portable_path};
use crate::strict_json;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const IMAGE_NAMES: [&str; 3] = ["engine", "frontend", "postgres"];

pub struct Expectations {
    pub version: Option<String>,
    pub revision: Option<String>,
    pub tag: Option<String>,
    pub source_state: Option<String>,
    pub image_mode: Option<String>,
}

pub fn validate_metadata(
    source: &[u8],
    package_root: &str,
    archive_name: &str,
    hashes: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let metadata_value = strict_json::from_slice(source)
        .map_err(|error| format!("cannot parse release metadata: {error}"))?;
    let metadata = object(&metadata_value, "release metadata")?;
    exact_keys(
        metadata,
        &[
            "schemaVersion",
            "package",
            "source",
            "compose",
            "images",
            "integrity",
        ],
        "release metadata",
    )?;
    if metadata["schemaVersion"].as_u64() != Some(1) {
        return Err("release metadata schemaVersion must be 1".to_owned());
    }
    let archive = parse_archive_name(archive_name)?;
    let package = object(&metadata["package"], "release metadata package")?;
    exact_keys(
        package,
        &["name", "version", "createdAt"],
        "release metadata package",
    )?;
    if package["name"].as_str() != Some(package_root) {
        return Err("release metadata package name does not match the archive".to_owned());
    }
    if !package["name"].as_str().is_some_and(valid_package_name) {
        return Err("release metadata package name is invalid".to_owned());
    }
    let version = package["version"].as_str().unwrap_or_default();
    if !decimal_triplet(version) {
        return Err("release metadata package version is invalid".to_owned());
    }
    if version != archive.version {
        return Err("release metadata package version does not match the archive".to_owned());
    }
    let created_at = parse_timestamp(&package["createdAt"], "release metadata package createdAt")?;
    if created_at != archive.timestamp {
        return Err("release metadata creation time does not match the archive name".to_owned());
    }

    validate_source(&metadata["source"], version)?;
    validate_compose(&metadata["compose"])?;
    validate_images(&metadata["images"], hashes)?;
    let integrity = object(&metadata["integrity"], "release metadata integrity")?;
    exact_keys(
        integrity,
        &["algorithm", "manifest"],
        "release metadata integrity",
    )?;
    if integrity["algorithm"].as_str() != Some("sha256")
        || integrity["manifest"].as_str() != Some("checksums.sha256")
    {
        return Err("release metadata integrity declaration is invalid".to_owned());
    }
    Ok(metadata_value)
}

pub fn enforce_expectations(metadata: &Value, expected: &Expectations) -> Result<(), String> {
    let package = object(&metadata["package"], "release metadata package")?;
    let source = object(&metadata["source"], "release metadata source")?;
    let images = object(&metadata["images"], "release metadata images")?;
    if let Some(version) = expected.version.as_deref() {
        if !decimal_triplet(version) {
            return Err("expected package version must use x.y.z".to_owned());
        }
        if package["version"].as_str() != Some(version) {
            return Err("candidate package version does not match expectation".to_owned());
        }
    }
    if let Some(revision) = expected.revision.as_deref() {
        let revision = revision.to_ascii_lowercase();
        if !lowercase_hex(&revision, &[40, 64]) {
            return Err("expected source revision is invalid".to_owned());
        }
        if source["revision"].as_str() != Some(&revision) {
            return Err("candidate source revision does not match expectation".to_owned());
        }
    }
    if let Some(tag) = expected.tag.as_deref() {
        if !tag.strip_prefix('v').is_some_and(decimal_triplet) {
            return Err("expected source Tag must use vx.y.z".to_owned());
        }
        if source["tag"].as_str() != Some(tag) {
            return Err("candidate source Tag does not match expectation".to_owned());
        }
    }
    if expected
        .source_state
        .as_deref()
        .is_some_and(|state| source["state"].as_str() != Some(state))
    {
        return Err("candidate source state does not match expectation".to_owned());
    }
    if expected
        .image_mode
        .as_deref()
        .is_some_and(|mode| images["mode"].as_str() != Some(mode))
    {
        return Err("candidate image mode does not match expectation".to_owned());
    }
    Ok(())
}

fn validate_source(value: &Value, version: &str) -> Result<(), String> {
    let source = object(value, "release metadata source")?;
    exact_keys(
        source,
        &["revision", "state", "tag"],
        "release metadata source",
    )?;
    let state = source["state"].as_str().unwrap_or_default();
    if !matches!(state, "clean" | "dirty" | "unavailable") {
        return Err("release metadata source state is invalid".to_owned());
    }
    if state == "unavailable" {
        if !source["revision"].is_null() || !source["tag"].is_null() {
            return Err("unavailable release source must not claim a revision or Tag".to_owned());
        }
    } else {
        if !source["revision"]
            .as_str()
            .is_some_and(|revision| lowercase_hex(revision, &[40, 64]))
        {
            return Err("release metadata source revision is invalid".to_owned());
        }
        let expected_tag = format!("v{version}");
        if !source["tag"].is_null() && source["tag"].as_str() != Some(&expected_tag) {
            return Err("release metadata source Tag does not match its version".to_owned());
        }
    }
    Ok(())
}

fn validate_compose(value: &Value) -> Result<(), String> {
    let compose = object(value, "release metadata compose")?;
    exact_keys(compose, &["file", "source"], "release metadata compose")?;
    if compose["file"].as_str() != Some("docker-compose.yml") {
        return Err("release metadata compose file is invalid".to_owned());
    }
    portable_path(&compose["source"], "release metadata compose source")?;
    Ok(())
}

fn validate_images(value: &Value, hashes: &BTreeMap<String, String>) -> Result<(), String> {
    let images = object(value, "release metadata images")?;
    exact_keys(
        images,
        &["mode", "references", "contentIds", "archive"],
        "release metadata images",
    )?;
    if !matches!(images["mode"].as_str(), Some("built" | "prebuilt")) {
        return Err("release metadata image mode is invalid".to_owned());
    }
    let references = object(&images["references"], "release metadata image references")?;
    let identities = object(&images["contentIds"], "release metadata image content IDs")?;
    exact_keys(
        references,
        &IMAGE_NAMES,
        "release metadata image references",
    )?;
    exact_keys(
        identities,
        &IMAGE_NAMES,
        "release metadata image content IDs",
    )?;
    for name in IMAGE_NAMES {
        nonempty_line(
            &references[name],
            &format!("release metadata {name} image reference"),
        )?;
        if !identities[name]
            .as_str()
            .and_then(|value| value.strip_prefix("sha256:"))
            .is_some_and(|hash| lowercase_hex(hash, &[64]))
        {
            return Err(format!(
                "release metadata {name} image content ID is invalid"
            ));
        }
    }
    let image_archive = object(&images["archive"], "release metadata image archive")?;
    exact_keys(
        image_archive,
        &["file", "sha256"],
        "release metadata image archive",
    )?;
    if image_archive["file"].as_str() != Some("cyanrex-images.tar") {
        return Err("release metadata image archive file is invalid".to_owned());
    }
    let archive_hash = image_archive["sha256"].as_str().unwrap_or_default();
    if !lowercase_hex(archive_hash, &[64]) {
        return Err("release metadata image archive SHA-256 is invalid".to_owned());
    }
    for required in ["docker-compose.yml", "cyanrex-images.tar"] {
        if !hashes.contains_key(required) {
            return Err(format!(
                "release metadata references missing package file {required}"
            ));
        }
    }
    if hashes.get("cyanrex-images.tar").map(String::as_str) != Some(archive_hash) {
        return Err("image archive SHA-256 does not match release metadata".to_owned());
    }
    Ok(())
}

fn parse_timestamp(value: &Value, label: &str) -> Result<DateTime<Utc>, String> {
    let text = value.as_str().unwrap_or_default();
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%SZ")
        .map(|timestamp| timestamp.and_utc())
        .map_err(|_| format!("{label} must be an RFC3339 UTC timestamp"))
}

fn exact_keys(value: &Map<String, Value>, keys: &[&str], label: &str) -> Result<(), String> {
    let expected = keys
        .iter()
        .map(|key| (*key).to_owned())
        .collect::<BTreeSet<_>>();
    let actual = value.keys().cloned().collect::<BTreeSet<_>>();
    if expected == actual {
        return Ok(());
    }
    let mut details = Vec::new();
    let missing = expected.difference(&actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(&expected).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        details.push(format!("missing {}", missing.join(", ")));
    }
    if !unexpected.is_empty() {
        details.push(format!("unexpected {}", unexpected.join(", ")));
    }
    Err(format!(
        "{label} fields are invalid: {}",
        details.join("; ")
    ))
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} must be a JSON object"))
}

fn nonempty_line(value: &Value, label: &str) -> Result<(), String> {
    let text = value.as_str().unwrap_or_default();
    if text.is_empty() || text.contains(['\n', '\r']) {
        Err(format!("{label} must be a non-empty single-line string"))
    } else {
        Ok(())
    }
}

fn valid_package_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
