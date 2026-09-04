use crate::{evidence_output, strict_json};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const IMAGE_NAMES: [&str; 3] = ["engine", "frontend", "postgres"];

pub struct CreateOptions {
    pub output: PathBuf,
    pub environment: PathBuf,
    pub event: PathBuf,
    pub program_name: String,
    pub pin_path: String,
    pub release_metadata: Option<PathBuf>,
}

pub struct VerifyOptions {
    pub report: PathBuf,
    pub release_metadata: Option<PathBuf>,
    pub expect_version: Option<String>,
    pub expect_revision: Option<String>,
    pub expect_tag: Option<String>,
    pub expect_source_state: Option<String>,
    pub expect_image_mode: Option<String>,
}

fn load_json(path: &Path, label: &str) -> Result<(Value, Vec<u8>), String> {
    let source = fs::read(path)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    let value = strict_json::from_slice(&source)
        .map_err(|error| format!("cannot read {label} {}: {error}", path.display()))?;
    if !value.is_object() {
        return Err(format!("{label} must be a JSON object"));
    }
    Ok((value, source))
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{label} must be a JSON object"))
}

fn exact_keys(value: &Map<String, Value>, keys: &[&str], label: &str) -> Result<(), String> {
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    let actual = value.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual == expected {
        return Ok(());
    }
    let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
    let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
    let mut details = Vec::new();
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

fn timestamp(value: &Value, label: &str) -> Result<(), String> {
    let invalid = || format!("{label} must be an RFC3339 UTC timestamp");
    let text = value.as_str().ok_or_else(invalid)?;
    let bytes = text.as_bytes();
    let base_digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let shape_ok = bytes.len() >= 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[bytes.len() - 1] == b'Z'
        && base_digits
            .iter()
            .all(|index| bytes[*index].is_ascii_digit())
        && (bytes.len() == 20
            || (bytes.len() > 21
                && bytes[19] == b'.'
                && bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)));
    if !shape_ok {
        return Err(invalid());
    }
    let parsed = DateTime::parse_from_rfc3339(text).map_err(|_| invalid())?;
    if parsed.offset().local_minus_utc() != 0 {
        return Err(invalid());
    }
    Ok(())
}

fn nonempty<'a>(value: &'a Value, label: &str) -> Result<&'a str, String> {
    let text = value.as_str().unwrap_or_default();
    if text.is_empty() || text.contains(['\n', '\r']) {
        return Err(format!("{label} must be a non-empty single-line string"));
    }
    Ok(text)
}

fn positive_integer(value: &Value, label: &str) -> Result<(), String> {
    if value.as_u64().is_some_and(|number| number > 0) {
        Ok(())
    } else {
        Err(format!("{label} must be a positive integer"))
    }
}

fn decimal_triplet(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn lowercase_hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn package_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn program_name(value: &str) -> bool {
    value
        .strip_prefix("release-kernel-smoke-")
        .is_some_and(|suffix| lowercase_hex(suffix, &[16]))
}

pub(crate) fn validate_environment(value: &Value) -> Result<(), String> {
    let report = object(value, "environment")?;
    if !report.get("overall_ok").is_some_and(Value::is_boolean) {
        return Err("environment overall_ok must be boolean".to_owned());
    }
    timestamp(&report["generated_at"], "environment generated_at")?;
    if !matches!(
        report.get("runtime_mode").and_then(Value::as_str),
        Some("native-linux" | "wsl2" | "docker")
    ) {
        return Err("environment runtime_mode is invalid".to_owned());
    }
    let checks = report
        .get("checks")
        .and_then(Value::as_array)
        .ok_or_else(|| "environment checks must be an array".to_owned())?;
    let kernel = checks.iter().find_map(|item| {
        item.as_object()
            .filter(|check| check.get("name").and_then(Value::as_str) == Some("kernel"))
    });
    let Some(kernel) = kernel else {
        return Err("environment must include a boolean kernel check".to_owned());
    };
    if !kernel.get("ok").is_some_and(Value::is_boolean) {
        return Err("environment must include a boolean kernel check".to_owned());
    }
    nonempty(&kernel["detail"], "environment kernel detail")?;
    Ok(())
}

fn validate_candidate(value: &Value) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let candidate = object(value, "candidate")?;
    exact_keys(
        candidate,
        &["releaseMetadataSha256", "package", "source", "images"],
        "candidate",
    )?;
    let metadata_hash = candidate["releaseMetadataSha256"]
        .as_str()
        .unwrap_or_default();
    if !lowercase_hex(metadata_hash, &[64]) {
        return Err("candidate release metadata SHA-256 is invalid".to_owned());
    }

    let package = object(&candidate["package"], "candidate package")?;
    exact_keys(
        package,
        &["name", "version", "createdAt"],
        "candidate package",
    )?;
    if !package["name"].as_str().is_some_and(package_name) {
        return Err("candidate package name is invalid".to_owned());
    }
    let version = package["version"].as_str().unwrap_or_default();
    if !decimal_triplet(version) {
        return Err("candidate package version is invalid".to_owned());
    }
    timestamp(&package["createdAt"], "candidate package createdAt")?;

    let source = object(&candidate["source"], "candidate source")?;
    exact_keys(source, &["revision", "state", "tag"], "candidate source")?;
    let state = source["state"].as_str().unwrap_or_default();
    if !matches!(state, "clean" | "dirty" | "unavailable") {
        return Err("candidate source state is invalid".to_owned());
    }
    if state == "unavailable" {
        if !source["revision"].is_null() || !source["tag"].is_null() {
            return Err("unavailable candidate source must not claim a revision or Tag".to_owned());
        }
    } else {
        if !source["revision"]
            .as_str()
            .is_some_and(|value| lowercase_hex(value, &[40, 64]))
        {
            return Err("candidate source revision is invalid".to_owned());
        }
        if !source["tag"].is_null()
            && source["tag"].as_str() != Some(format!("v{version}").as_str())
        {
            return Err("candidate source Tag does not match its package version".to_owned());
        }
    }

    let images = object(&candidate["images"], "candidate images")?;
    exact_keys(
        images,
        &["mode", "references", "contentIds", "archiveSha256"],
        "candidate images",
    )?;
    if !matches!(images["mode"].as_str(), Some("built" | "prebuilt")) {
        return Err("candidate image mode is invalid".to_owned());
    }
    let references = object(&images["references"], "candidate image references")?;
    let content_ids = object(&images["contentIds"], "candidate image content IDs")?;
    exact_keys(references, &IMAGE_NAMES, "candidate image references")?;
    exact_keys(content_ids, &IMAGE_NAMES, "candidate image content IDs")?;
    for name in IMAGE_NAMES {
        nonempty(
            &references[name],
            &format!("candidate {name} image reference"),
        )?;
        let content_id = content_ids[name].as_str().unwrap_or_default();
        if !content_id
            .strip_prefix("sha256:")
            .is_some_and(|hash| lowercase_hex(hash, &[64]))
        {
            return Err(format!("candidate {name} image content ID is invalid"));
        }
    }
    if !images["archiveSha256"]
        .as_str()
        .is_some_and(|value| lowercase_hex(value, &[64]))
    {
        return Err("candidate image archive SHA-256 is invalid".to_owned());
    }
    Ok(())
}

pub(crate) fn candidate_from_metadata(path: &Path) -> Result<Value, String> {
    let (metadata_value, source) = load_json(path, "release metadata")?;
    let metadata = object(&metadata_value, "release metadata")?;
    if metadata.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("release metadata schemaVersion must be 1".to_owned());
    }
    let images = object(&metadata["images"], "release metadata images")?;
    let archive = object(&images["archive"], "release metadata image archive")?;
    let candidate = json!({
        "releaseMetadataSha256": sha256(&source),
        "package": metadata.get("package").cloned().unwrap_or(Value::Null),
        "source": metadata.get("source").cloned().unwrap_or(Value::Null),
        "images": {
            "mode": images.get("mode").cloned().unwrap_or(Value::Null),
            "references": images.get("references").cloned().unwrap_or(Value::Null),
            "contentIds": images.get("contentIds").cloned().unwrap_or(Value::Null),
            "archiveSha256": archive.get("sha256").cloned().unwrap_or(Value::Null),
        },
    });
    validate_candidate(&candidate)?;
    Ok(candidate)
}

fn sha256(source: &[u8]) -> String {
    Sha256::digest(source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn validate_report(value: &Value) -> Result<(), String> {
    let report = object(value, "live kernel evidence")?;
    exact_keys(
        report,
        &[
            "schemaVersion",
            "result",
            "generatedAt",
            "candidate",
            "environment",
            "exercise",
            "cleanup",
        ],
        "live kernel evidence",
    )?;
    let schema_version = report["schemaVersion"].as_u64();
    if !matches!(schema_version, Some(1 | 2)) || report["result"].as_str() != Some("passed") {
        return Err("live kernel evidence must report a passed supported schema result".to_owned());
    }
    timestamp(&report["generatedAt"], "evidence generatedAt")?;
    validate_candidate(&report["candidate"])?;
    validate_environment(&report["environment"])?;

    let exercise = object(&report["exercise"], "exercise")?;
    exact_keys(
        exercise,
        &[
            "templateId",
            "programName",
            "runtimeBackend",
            "hook",
            "pinPath",
            "event",
        ],
        "exercise",
    )?;
    if exercise["templateId"].as_str() != Some("ringbuf-hi-freq-sampler") {
        return Err("exercise template is not the release ring-buffer sampler".to_owned());
    }
    let exercise_program = exercise["programName"].as_str().unwrap_or_default();
    if !program_name(exercise_program) {
        return Err("exercise program name is not uniquely bound".to_owned());
    }
    if exercise["runtimeBackend"].as_str() != Some("aya") {
        return Err("exercise runtime backend must be aya".to_owned());
    }
    if exercise["hook"].as_str() != Some("tracepoint/sched/sched_switch") {
        return Err("exercise hook must be sched_switch".to_owned());
    }
    if !nonempty(&exercise["pinPath"], "exercise pin path")?.starts_with("/sys/fs/bpf/") {
        return Err("exercise pin path must be inside bpffs".to_owned());
    }

    let event = object(&exercise["event"], "exercise event")?;
    let event_fields = if schema_version == Some(2) {
        vec!["timestamp", "type", "bytes", "programName"]
    } else {
        vec!["timestamp", "type", "bytes"]
    };
    exact_keys(event, &event_fields, "exercise event")?;
    timestamp(&event["timestamp"], "exercise event timestamp")?;
    if event["type"].as_str() != Some("ebpf.kernel_ringbuf") {
        return Err("exercise event type is invalid".to_owned());
    }
    positive_integer(&event["bytes"], "exercise event bytes")?;
    if schema_version == Some(2) && event["programName"].as_str() != Some(exercise_program) {
        return Err("exercise event is not bound to its unique program name".to_owned());
    }

    let cleanup = object(&report["cleanup"], "cleanup")?;
    exact_keys(
        cleanup,
        &["exactPinDetached", "remainingAttachments"],
        "cleanup",
    )?;
    if cleanup["exactPinDetached"].as_bool() != Some(true)
        || cleanup["remainingAttachments"].as_u64() != Some(0)
    {
        return Err("live kernel evidence does not prove exact cleanup".to_owned());
    }
    Ok(())
}

fn create_report(options: &CreateOptions) -> Result<Value, String> {
    let (environment, _) = load_json(&options.environment, "environment report")?;
    let (event_value, _) = load_json(&options.event, "matched kernel event")?;
    build_report(
        environment,
        event_value,
        &options.program_name,
        &options.pin_path,
        options.release_metadata.as_deref(),
    )
}

fn build_report(
    environment: Value,
    event_value: Value,
    program_name: &str,
    pin_path: &str,
    release_metadata: Option<&Path>,
) -> Result<Value, String> {
    validate_environment(&environment)?;
    let event = object(&event_value, "matched kernel event")?;
    let payload = object(&event["payload"], "matched kernel event payload")?;
    if event["event_type"].as_str() != Some("ebpf.kernel_ringbuf") {
        return Err("matched kernel event type is invalid".to_owned());
    }
    if payload["program_name"].as_str() != Some(program_name) {
        return Err("matched kernel event belongs to another program".to_owned());
    }
    positive_integer(&payload["bytes"], "matched kernel event bytes")?;
    timestamp(&event["timestamp"], "matched kernel event timestamp")?;
    let candidate = release_metadata
        .map(candidate_from_metadata)
        .transpose()?
        .unwrap_or(Value::Null);
    let report = json!({
        "schemaVersion": 2,
        "result": "passed",
        "generatedAt": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "candidate": candidate,
        "environment": environment,
        "exercise": {
            "templateId": "ringbuf-hi-freq-sampler",
            "programName": program_name,
            "runtimeBackend": "aya",
            "hook": "tracepoint/sched/sched_switch",
            "pinPath": pin_path,
            "event": {
                "timestamp": event.get("timestamp").cloned().unwrap_or(Value::Null),
                "type": event.get("event_type").cloned().unwrap_or(Value::Null),
                "bytes": payload.get("bytes").cloned().unwrap_or(Value::Null),
                "programName": payload.get("program_name").cloned().unwrap_or(Value::Null),
            },
        },
        "cleanup": {"exactPinDetached": true, "remainingAttachments": 0},
    });
    validate_report(&report)?;
    Ok(report)
}

pub fn create(options: &CreateOptions) -> Result<PathBuf, String> {
    let report = create_report(options)?;
    evidence_output::write_report(&report, &options.output)
}

pub(crate) fn create_from_values(
    environment: Value,
    event: Value,
    program_name: &str,
    pin_path: &str,
    release_metadata: Option<&Path>,
    output: &Path,
) -> Result<PathBuf, String> {
    let report = build_report(environment, event, program_name, pin_path, release_metadata)?;
    evidence_output::write_report(&report, output)
}

pub fn verify(options: &VerifyOptions) -> Result<(), String> {
    let (report_value, _) = load_json(&options.report, "live kernel evidence")?;
    validate_report(&report_value)?;
    if options
        .expect_version
        .as_deref()
        .is_some_and(|value| !decimal_triplet(value))
    {
        return Err("expected package version must use x.y.z".to_owned());
    }
    if options
        .expect_revision
        .as_deref()
        .is_some_and(|value| !lowercase_hex(value, &[40, 64]))
    {
        return Err("expected source revision is invalid".to_owned());
    }
    if options
        .expect_tag
        .as_deref()
        .is_some_and(|value| !value.strip_prefix('v').is_some_and(decimal_triplet))
    {
        return Err("expected source Tag must use vx.y.z".to_owned());
    }
    if options
        .expect_source_state
        .as_deref()
        .is_some_and(|value| !matches!(value, "clean" | "dirty" | "unavailable"))
    {
        return Err("expected source state must be clean, dirty, or unavailable".to_owned());
    }
    if options
        .expect_image_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "built" | "prebuilt"))
    {
        return Err("expected image mode must be built or prebuilt".to_owned());
    }
    let report = object(&report_value, "live kernel evidence")?;
    let candidate = &report["candidate"];
    if let Some(metadata_path) = options.release_metadata.as_deref() {
        let expected = candidate_from_metadata(metadata_path)?;
        if candidate != &expected {
            return Err(
                "live kernel evidence does not match the supplied release metadata".to_owned(),
            );
        }
    }
    let has_expectations = options.expect_version.is_some()
        || options.expect_revision.is_some()
        || options.expect_tag.is_some()
        || options.expect_source_state.is_some()
        || options.expect_image_mode.is_some();
    if has_expectations && candidate.is_null() {
        return Err("candidate expectations require release-bound evidence".to_owned());
    }
    if !candidate.is_null() {
        let candidate = object(candidate, "candidate")?;
        let package = object(&candidate["package"], "candidate package")?;
        let source = object(&candidate["source"], "candidate source")?;
        if options
            .expect_version
            .as_deref()
            .is_some_and(|expected| package["version"].as_str() != Some(expected))
        {
            return Err(
                "live kernel evidence package version does not match expectation".to_owned(),
            );
        }
        if options
            .expect_revision
            .as_deref()
            .is_some_and(|expected| source["revision"].as_str() != Some(expected))
        {
            return Err(
                "live kernel evidence source revision does not match expectation".to_owned(),
            );
        }
        if options
            .expect_tag
            .as_deref()
            .is_some_and(|expected| source["tag"].as_str() != Some(expected))
        {
            return Err("live kernel evidence source Tag does not match expectation".to_owned());
        }
        if options
            .expect_source_state
            .as_deref()
            .is_some_and(|expected| source["state"].as_str() != Some(expected))
        {
            return Err("live kernel evidence source state does not match expectation".to_owned());
        }
        if options
            .expect_image_mode
            .as_deref()
            .is_some_and(|expected| candidate["images"]["mode"].as_str() != Some(expected))
        {
            return Err("live kernel evidence image mode does not match expectation".to_owned());
        }
    }
    Ok(())
}
