use crate::release_archive::{
    archive_path, decimal_triplet, inspect_archive, regular_file, verify_checksum_file,
    verify_manifest, MAX_ARCHIVE_BYTES, MAX_ARCHIVE_MEMBERS,
};
use crate::release_metadata::{enforce_expectations, validate_metadata, Expectations};
use flate2::read::GzDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct ExtractedPackage {
    pub metadata: Value,
    pub path: PathBuf,
}

struct VerifiedPackage {
    metadata: Value,
    archive: PathBuf,
    package_root: String,
    hashes: BTreeMap<String, String>,
}

pub fn verify_and_extract(
    bundle: &Path,
    output: &Path,
    expectations: &Expectations,
) -> Result<ExtractedPackage, String> {
    let verified = verify_package(bundle, expectations)?;
    let path = extract_archive(
        &verified.archive,
        &verified.package_root,
        &verified.hashes,
        output,
    )?;
    Ok(ExtractedPackage {
        metadata: verified.metadata,
        path,
    })
}

fn verify_package(bundle: &Path, expectations: &Expectations) -> Result<VerifiedPackage, String> {
    let archive = discover_package_bundle(bundle)?;
    let archive_name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "release package archive name is not UTF-8".to_owned())?;
    let inspected = inspect_archive(&archive)?;
    verify_manifest(
        &inspected.hashes,
        inspected
            .controls
            .get("checksums.sha256")
            .expect("archive inspection requires its checksum manifest"),
    )?;
    let metadata = validate_metadata(
        inspected
            .controls
            .get("release-metadata.json")
            .expect("archive inspection requires release metadata"),
        &inspected.package_root,
        archive_name,
        &inspected.hashes,
    )?;
    enforce_expectations(&metadata, expectations)?;
    Ok(VerifiedPackage {
        metadata,
        archive,
        package_root: inspected.package_root,
        hashes: inspected.hashes,
    })
}

fn discover_package_bundle(directory: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(directory).map_err(|error| {
        format!(
            "cannot inspect release package bundle {}: {error}",
            directory.display()
        )
    })?;
    if !root.is_dir() {
        return Err(format!(
            "release package bundle is not a directory: {}",
            directory.display()
        ));
    }
    let entries = fs::read_dir(&root).map_err(|error| {
        format!(
            "cannot inspect release package bundle {}: {error}",
            directory.display()
        )
    })?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "cannot inspect release package bundle {}: {error}",
                directory.display()
            )
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "release package bundle contains a non-UTF-8 file name".to_owned())?;
        names.insert(name);
    }
    let archives = names
        .iter()
        .filter(|name| archive_name_syntax(name))
        .cloned()
        .collect::<Vec<_>>();
    if archives.len() != 1 {
        return Err(
            "release package bundle must contain exactly one Cyanrex .tar.gz archive".to_owned(),
        );
    }
    let archive_name = &archives[0];
    let expected = [archive_name.clone(), format!("{archive_name}.sha256")]
        .into_iter()
        .collect::<BTreeSet<_>>();
    if names != expected {
        return Err(file_set_error(
            "release package bundle file set is invalid",
            &expected,
            &names,
        ));
    }
    let archive = root.join(archive_name);
    regular_file(&archive, "release package archive", None)?;
    verify_checksum_file(&root.join(format!("{archive_name}.sha256")), &archive)?;
    Ok(archive)
}

fn extract_archive(
    archive_path: &Path,
    package_root: &str,
    hashes: &BTreeMap<String, String>,
    requested_output: &Path,
) -> Result<PathBuf, String> {
    let destination = output_path(requested_output)?;
    create_private_directory(&destination)?;
    let parent = destination
        .parent()
        .expect("resolved extraction output always has a parent");
    let name = destination
        .file_name()
        .expect("resolved extraction output always has a name")
        .to_string_lossy();
    let temporary = parent.join(format!(".{name}.extract-{}", Uuid::new_v4().simple()));
    if let Err(error) = create_private_directory(&temporary) {
        let _ = fs::remove_dir(&destination);
        return Err(error);
    }
    let result = extract_to_temporary(archive_path, &temporary, package_root, hashes).and_then(
        |temporary_package| {
            set_directory_mode(&destination, 0o755)?;
            let final_package = destination.join(package_root);
            if path_exists(&final_package)? {
                return Err(format!(
                    "refusing to overwrite extracted package: {}",
                    final_package.display()
                ));
            }
            fs::rename(&temporary_package, &final_package).map_err(|error| {
                format!(
                    "cannot publish extracted package {}: {error}",
                    final_package.display()
                )
            })?;
            fs::remove_dir(&temporary).map_err(|error| {
                format!(
                    "cannot remove extraction staging directory {}: {error}",
                    temporary.display()
                )
            })?;
            Ok(final_package)
        },
    );
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
        let _ = fs::remove_dir(&destination);
    }
    result
}

fn extract_to_temporary(
    archive_file_path: &Path,
    temporary: &Path,
    package_root: &str,
    hashes: &BTreeMap<String, String>,
) -> Result<PathBuf, String> {
    let file = File::open(archive_file_path)
        .map_err(|error| format!("cannot open candidate archive: {error}"))?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let allowed_directories = allowed_directories(package_root, hashes);
    let mut seen = BTreeSet::new();
    let mut extracted = BTreeSet::new();
    let mut total_size = 0_u64;
    let mut root_seen = false;
    let entries = archive
        .entries()
        .map_err(|error| format!("cannot safely extract candidate archive: {error}"))?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ARCHIVE_MEMBERS {
            return Err(format!(
                "archive exceeds the {MAX_ARCHIVE_MEMBERS}-member limit"
            ));
        }
        let mut entry =
            entry.map_err(|error| format!("cannot safely extract candidate archive: {error}"))?;
        let entry_type = entry.header().entry_type();
        let raw_name = String::from_utf8(entry.path_bytes().into_owned())
            .map_err(|_| "archive member path is not UTF-8".to_owned())?;
        let (name, parts) = archive_path(&raw_name, entry_type.is_dir())?;
        if parts[0] != package_root {
            return Err(format!(
                "archive member is outside {package_root}: {raw_name:?}"
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(format!("archive contains duplicate member {name:?}"));
        }
        let destination = parts
            .iter()
            .fold(temporary.to_path_buf(), |path, part| path.join(part));
        if entry_type.is_dir() {
            if !allowed_directories.contains(&name) {
                return Err(format!("archive contains unexpected directory {name:?}"));
            }
            fs::create_dir_all(&destination)
                .map_err(|error| format!("cannot create archive directory {name:?}: {error}"))?;
            if !fs::symlink_metadata(&destination)
                .map(|metadata| metadata.file_type().is_dir())
                .unwrap_or(false)
            {
                return Err(format!("archive directory conflicts with a file: {name:?}"));
            }
            set_directory_mode(&destination, 0o755)?;
            root_seen |= parts.len() == 1;
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
        let relative = parts[1..].join("/");
        let expected_hash = hashes
            .get(&relative)
            .ok_or_else(|| format!("archive contains unverified member {raw_name:?}"))?;
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
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create archive parent {relative:?}: {error}"))?;
        }
        let mode = entry.header().mode().unwrap_or(0o644);
        write_member(&mut entry, &destination, &raw_name, size, expected_hash)?;
        set_file_mode(&destination, if mode & 0o111 == 0 { 0o644 } else { 0o755 })?;
        extracted.insert(relative);
    }
    if !root_seen {
        return Err("archive does not contain its declared package root directory".to_owned());
    }
    let expected = hashes.keys().cloned().collect::<BTreeSet<_>>();
    if extracted != expected {
        let missing = expected.difference(&extracted).cloned().collect::<Vec<_>>();
        return Err(format!(
            "verified package members disappeared before extraction: {}",
            missing.join(", ")
        ));
    }
    Ok(temporary.join(package_root))
}

fn write_member(
    reader: &mut impl Read,
    destination: &Path,
    archive_name: &str,
    expected_size: u64,
    expected_hash: &str,
) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut output = options
        .open(destination)
        .map_err(|error| format!("cannot write archive member {archive_name:?}: {error}"))?;
    let mut digest = Sha256::new();
    let mut actual_size = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("cannot read archive member {archive_name:?}: {error}"))?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(|error| format!("cannot write archive member {archive_name:?}: {error}"))?;
        digest.update(&buffer[..count]);
        actual_size += count as u64;
    }
    if actual_size != expected_size {
        return Err(format!(
            "archive member size changed during extraction: {archive_name:?}"
        ));
    }
    let actual_hash = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual_hash != expected_hash {
        return Err(format!(
            "archive member changed after verification: {archive_name:?}"
        ));
    }
    Ok(())
}

fn output_path(value: &Path) -> Result<PathBuf, String> {
    let requested = if value.is_absolute() {
        value.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve extraction output: {error}"))?
            .join(value)
    };
    let name = requested
        .file_name()
        .ok_or_else(|| "extraction output must name a new directory".to_owned())?;
    let parent = requested.parent().ok_or_else(|| {
        format!(
            "extraction output parent does not exist: {}",
            requested.display()
        )
    })?;
    let parent = fs::canonicalize(parent).map_err(|_| {
        format!(
            "extraction output parent does not exist: {}",
            parent.display()
        )
    })?;
    let destination = parent.join(name);
    if path_exists(&destination)? {
        return Err(format!(
            "refusing to overwrite extraction output: {}",
            destination.display()
        ));
    }
    Ok(destination)
}

fn archive_name_syntax(name: &str) -> bool {
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

fn allowed_directories(package_root: &str, hashes: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut directories = BTreeSet::from([package_root.to_owned()]);
    for relative in hashes.keys() {
        let parts = relative.split('/').collect::<Vec<_>>();
        for length in 1..parts.len() {
            directories.insert(format!("{package_root}/{}", parts[..length].join("/")));
        }
    }
    directories
}

fn file_set_error(label: &str, expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> String {
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

fn create_private_directory(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|error| {
        format!(
            "cannot create extraction directory {}: {error}",
            path.display()
        )
    })
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect {}: {error}", path.display())),
    }
}

#[cfg(unix)]
fn set_directory_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
        format!(
            "cannot set directory permissions {}: {error}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_directory_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("cannot set file permissions {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}
