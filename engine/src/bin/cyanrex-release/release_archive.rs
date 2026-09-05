use chrono::{DateTime, NaiveDateTime, Utc};
use flate2::read::GzDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub const MAX_ARCHIVE_MEMBERS: usize = 512;
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_CONTROL_BYTES: u64 = 4 * 1024 * 1024;

pub struct ArchiveInspection {
    pub package_root: String,
    pub hashes: BTreeMap<String, String>,
    pub controls: BTreeMap<String, Vec<u8>>,
}

pub struct ArchiveName {
    pub package_root: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
}

pub fn inspect_archive(archive_file_path: &Path) -> Result<ArchiveInspection, String> {
    let archive_file_name = file_name(archive_file_path)?;
    let identity = parse_archive_name(&archive_file_name)?;
    let file = File::open(archive_file_path)
        .map_err(|error| format!("cannot open candidate archive {archive_file_name}: {error}"))?;
    let decoder = GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut seen = BTreeSet::new();
    let mut hashes = BTreeMap::new();
    let mut controls = BTreeMap::new();
    let mut total_size = 0_u64;
    let mut root_directory_seen = false;
    let entries = archive.entries().map_err(|error| {
        format!("cannot inspect candidate archive {archive_file_name}: {error}")
    })?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ARCHIVE_MEMBERS {
            return Err(format!(
                "archive exceeds the {MAX_ARCHIVE_MEMBERS}-member limit"
            ));
        }
        let mut entry = entry.map_err(|error| {
            format!("cannot inspect candidate archive {archive_file_name}: {error}")
        })?;
        let entry_type = entry.header().entry_type();
        let raw_name = String::from_utf8(entry.path_bytes().into_owned())
            .map_err(|_| "archive member path is not UTF-8".to_owned())?;
        let (name, parts) = archive_path(&raw_name, entry_type.is_dir())?;
        if parts[0] != identity.package_root {
            return Err(format!(
                "archive member is outside {}: {raw_name:?}",
                identity.package_root
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(format!("archive contains duplicate member {name:?}"));
        }
        if entry_type.is_dir() {
            root_directory_seen |= parts.len() == 1;
            continue;
        }
        if !entry_type.is_file() {
            return Err(format!(
                "archive contains unsupported member type: {raw_name:?}"
            ));
        }
        if parts.len() == 1 {
            return Err("archive package root must be a directory".to_owned());
        }
        let size = entry
            .header()
            .size()
            .map_err(|error| format!("archive member size is invalid: {raw_name:?}: {error}"))?;
        total_size = total_size
            .checked_add(size)
            .ok_or_else(|| format!("archive exceeds the {MAX_ARCHIVE_BYTES}-byte content limit"))?;
        if total_size > MAX_ARCHIVE_BYTES {
            return Err(format!(
                "archive exceeds the {MAX_ARCHIVE_BYTES}-byte content limit"
            ));
        }
        let relative = parts[1..].join("/");
        if hashes.contains_key(&relative) {
            return Err(format!(
                "archive contains duplicate package path {relative:?}"
            ));
        }
        let capture = matches!(
            relative.as_str(),
            "checksums.sha256" | "release-metadata.json"
        );
        if capture && size > MAX_CONTROL_BYTES {
            return Err(format!("archive control file is too large: {relative}"));
        }
        let (digest, source, actual_size) = hash_reader(&mut entry, capture)
            .map_err(|error| format!("cannot read archive member {raw_name:?}: {error}"))?;
        if actual_size != size {
            return Err(format!("archive member size is inconsistent: {raw_name:?}"));
        }
        hashes.insert(relative.clone(), digest);
        if let Some(source) = source {
            controls.insert(relative, source);
        }
    }
    if !root_directory_seen {
        return Err("archive does not contain its declared package root directory".to_owned());
    }
    for required in ["checksums.sha256", "release-metadata.json"] {
        if !controls.contains_key(required) {
            return Err(format!("archive is missing {required}"));
        }
    }
    Ok(ArchiveInspection {
        package_root: identity.package_root,
        hashes,
        controls,
    })
}

pub fn verify_manifest(hashes: &BTreeMap<String, String>, source: &[u8]) -> Result<(), String> {
    let manifest = parse_manifest(source)?;
    let expected = hashes
        .keys()
        .filter(|name| name.as_str() != "checksums.sha256")
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual = manifest.keys().cloned().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(set_error(
            "checksum manifest file set is invalid",
            &expected,
            &actual,
        ));
    }
    for (name, expected_hash) in manifest {
        if hashes.get(&name) != Some(&expected_hash) {
            return Err(format!("checksum manifest does not match {name}"));
        }
    }
    Ok(())
}

pub fn regular_file(path: &Path, label: &str, maximum: Option<u64>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "{label} must be a regular file: {}",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default()
        ));
    }
    if maximum.is_some_and(|limit| metadata.len() > limit) {
        return Err(format!(
            "{label} exceeds the {}-byte limit",
            maximum.unwrap()
        ));
    }
    Ok(())
}

pub fn verify_checksum_file(checksum: &Path, target: &Path) -> Result<(), String> {
    regular_file(checksum, "checksum file", Some(4096))?;
    let source = fs::read(checksum)
        .map_err(|error| format!("cannot read checksum file {}: {error}", checksum.display()))?;
    let target_name = file_name(target)?;
    let line = source.strip_suffix(b"\n").unwrap_or(&source);
    let entry = parse_checksum_line(line);
    if source
        .strip_suffix(b"\n")
        .is_some_and(|value| value.ends_with(b"\n"))
        || entry.as_ref().map(|(_, name)| *name) != Some(target_name.as_str())
    {
        return Err(format!(
            "checksum file must contain exactly one entry for {target_name}"
        ));
    }
    let expected_hash = entry.expect("validated checksum entry").0;
    if sha256_file(target)? != expected_hash {
        return Err(format!("SHA-256 mismatch for {target_name}"));
    }
    Ok(())
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    hash_reader(&mut file, false)
        .map(|result| result.0)
        .map_err(|error| format!("cannot hash {}: {error}", path.display()))
}

pub fn archive_path(raw_name: &str, directory: bool) -> Result<(String, Vec<String>), String> {
    let name = if directory {
        raw_name.strip_suffix('/').unwrap_or(raw_name)
    } else {
        raw_name
    };
    if name.is_empty() || name.contains('\\') || name.starts_with('/') {
        return Err(format!("archive member has an unsafe path: {raw_name:?}"));
    }
    let parts = name.split('/').map(str::to_owned).collect::<Vec<_>>();
    let invalid = parts.iter().any(|part| {
        part.is_empty()
            || matches!(part.as_str(), "." | "..")
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    }) || parts[0].contains(':');
    if invalid {
        return Err(format!(
            "archive member escapes its package root: {raw_name:?}"
        ));
    }
    Ok((name.to_owned(), parts))
}

pub(crate) fn parse_manifest(source: &[u8]) -> Result<BTreeMap<String, String>, String> {
    std::str::from_utf8(source)
        .map_err(|error| format!("checksum manifest is not UTF-8: {error}"))?;
    let mut entries = BTreeMap::new();
    for line in source
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let (hash, target) = parse_checksum_line(line).ok_or_else(|| {
            format!(
                "invalid checksum manifest line: {:?}",
                String::from_utf8_lossy(line)
            )
        })?;
        let target = portable_path(&Value::String(target.to_owned()), "checksum target")?;
        if entries.insert(target.clone(), hash.to_owned()).is_some() {
            return Err(format!("duplicate checksum entry for {target}"));
        }
    }
    if entries.is_empty() {
        return Err("checksum manifest is empty".to_owned());
    }
    Ok(entries)
}

fn parse_checksum_line(line: &[u8]) -> Option<(&str, &str)> {
    if line.len() <= 66
        || !line[..64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        || line[64] != b' '
        || !matches!(line[65], b' ' | b'*')
        || line[66..].contains(&b'\r')
        || line[66..].contains(&b'\n')
    {
        return None;
    }
    Some((
        std::str::from_utf8(&line[..64]).ok()?,
        std::str::from_utf8(&line[66..]).ok()?,
    ))
}

pub fn parse_archive_name(name: &str) -> Result<ArchiveName, String> {
    let version_and_timestamp = name
        .strip_suffix(".tar.gz")
        .and_then(|value| value.strip_prefix("cyanrex-lab-"))
        .ok_or_else(|| format!("invalid candidate archive name: {name}"))?;
    if version_and_timestamp.len() <= 16 {
        return Err(format!("invalid candidate archive name: {name}"));
    }
    let timestamp_separator = version_and_timestamp.len() - 16;
    if version_and_timestamp.as_bytes()[timestamp_separator] != b'-' {
        return Err(format!("invalid candidate archive name: {name}"));
    }
    let version = &version_and_timestamp[..timestamp_separator];
    let timestamp = &version_and_timestamp[timestamp_separator + 1..];
    let package_root = name.trim_end_matches(".tar.gz");
    if !decimal_triplet(version)
        || timestamp.len() != 15
        || timestamp.as_bytes()[8] != b'-'
        || !timestamp
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 8 || byte.is_ascii_digit())
    {
        return Err(format!("invalid candidate archive name: {name}"));
    }
    let timestamp = NaiveDateTime::parse_from_str(timestamp, "%Y%m%d-%H%M%S")
        .map_err(|_| "candidate archive timestamp is invalid".to_owned())?
        .and_utc();
    Ok(ArchiveName {
        package_root: package_root.to_owned(),
        version: version.to_owned(),
        timestamp,
    })
}

pub fn archive_name_syntax(name: &str) -> bool {
    let Some(value) = name
        .strip_suffix(".tar.gz")
        .and_then(|value| value.strip_prefix("cyanrex-lab-"))
    else {
        return false;
    };
    if value.len() <= 16 {
        return false;
    }
    let timestamp_separator = value.len() - 16;
    if value.as_bytes()[timestamp_separator] != b'-' {
        return false;
    }
    let version = &value[..timestamp_separator];
    let timestamp = &value[timestamp_separator + 1..];
    decimal_triplet(version)
        && timestamp.len() == 15
        && timestamp.as_bytes()[8] == b'-'
        && timestamp
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 8 || byte.is_ascii_digit())
}

pub fn portable_path(value: &Value, label: &str) -> Result<String, String> {
    let text = value.as_str().unwrap_or_default();
    if text.is_empty() || text.contains('\\') || text.starts_with('/') {
        return Err(format!("{label} must be a portable relative path"));
    }
    let parts = text.split('/').collect::<Vec<_>>();
    let invalid = parts.iter().any(|part| {
        part.is_empty()
            || matches!(*part, "." | "..")
            || !part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    }) || parts[0].contains(':');
    if invalid {
        return Err(format!("{label} must stay within the package"));
    }
    Ok(text.to_owned())
}

fn set_error(label: &str, expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> String {
    let mut details = Vec::new();
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(expected).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        details.push(format!("missing {}", missing.join(", ")));
    }
    if !unexpected.is_empty() {
        details.push(format!("unexpected {}", unexpected.join(", ")));
    }
    format!("{label}: {}", details.join("; "))
}

pub fn decimal_triplet(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn lowercase_hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("path has no portable file name: {}", path.display()))
}

fn hash_reader(
    reader: &mut impl Read,
    capture: bool,
) -> io::Result<(String, Option<Vec<u8>>, u64)> {
    let mut digest = Sha256::new();
    let mut captured = capture.then(Vec::new);
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        size += count as u64;
        if let Some(source) = captured.as_mut() {
            source.extend_from_slice(&buffer[..count]);
        }
    }
    let hash = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((hash, captured, size))
}
