use crate::release_archive::{
    parse_manifest, portable_path, verify_manifest, MAX_ARCHIVE_BYTES, MAX_ARCHIVE_MEMBERS,
    MAX_CONTROL_BYTES,
};
use crate::release_metadata::validate_metadata;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

pub fn verify(root: &Path) -> Result<Value, String> {
    let (root, package_name, archive_name) = package_identity(root)?;
    let mut hashes = BTreeMap::new();
    let mut controls = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![(root.clone(), String::new())];
    let mut members = 0_usize;
    let mut total_size = 0_u64;
    while let Some((directory, prefix)) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| {
            format!(
                "cannot inspect installed package directory {}: {error}",
                directory.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "cannot inspect installed package directory {}: {error}",
                    directory.display()
                )
            })?;
            members += 1;
            if members > MAX_ARCHIVE_MEMBERS {
                return Err(format!(
                    "installed package exceeds the {MAX_ARCHIVE_MEMBERS}-member limit"
                ));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "installed package contains a non-UTF-8 path".to_owned())?;
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            portable_path(&Value::String(relative.clone()), "installed package path")?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
                format!("cannot inspect installed package member {relative:?}: {error}")
            })?;
            if metadata.file_type().is_dir() {
                directories.insert(relative.clone());
                pending.push((entry.path(), relative));
                continue;
            }
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "installed package contains a non-regular member: {relative:?}"
                ));
            }
            total_size = total_size.checked_add(metadata.len()).ok_or_else(|| {
                format!("installed package exceeds the {MAX_ARCHIVE_BYTES}-byte content limit")
            })?;
            if total_size > MAX_ARCHIVE_BYTES {
                return Err(format!(
                    "installed package exceeds the {MAX_ARCHIVE_BYTES}-byte content limit"
                ));
            }
            let capture = matches!(
                relative.as_str(),
                "checksums.sha256" | "release-metadata.json"
            );
            if capture && metadata.len() > MAX_CONTROL_BYTES {
                return Err(format!(
                    "installed package control file is too large: {relative}"
                ));
            }
            let (hash, source, actual_size) = hash_regular_file(&entry.path(), capture)?;
            if actual_size != metadata.len() {
                return Err(format!(
                    "installed package member changed while being verified: {relative:?}"
                ));
            }
            hashes.insert(relative.clone(), hash);
            if let Some(source) = source {
                controls.insert(relative, source);
            }
        }
    }
    let manifest = controls
        .get("checksums.sha256")
        .ok_or_else(|| "installed package is missing checksums.sha256".to_owned())?;
    verify_manifest(&hashes, manifest)?;
    let expected_directories = manifest_directories(hashes.keys());
    if directories != expected_directories {
        return Err(set_error(
            "installed package directory set is invalid",
            &expected_directories,
            &directories,
        ));
    }
    let metadata = controls
        .get("release-metadata.json")
        .ok_or_else(|| "installed package is missing release-metadata.json".to_owned())?;
    validate_metadata(metadata, &package_name, &archive_name, &hashes)
}

pub fn verify_loaded_images(root: &Path) -> Result<(), String> {
    let metadata = load_control_metadata(root)?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot initialize image inspection runtime: {error}"))?
        .block_on(crate::installed_images::verify(&metadata))
}

fn load_control_metadata(root: &Path) -> Result<Value, String> {
    let (root, package_name, archive_name) = package_identity(root)?;
    let manifest_path = root.join("checksums.sha256");
    let metadata_path = root.join("release-metadata.json");
    let (_, manifest_source, _) = hash_regular_file(&manifest_path, true)?;
    let (metadata_hash, metadata_source, _) = hash_regular_file(&metadata_path, true)?;
    let manifest =
        parse_manifest(&manifest_source.expect("captured installed package checksum manifest"))?;
    if manifest.get("release-metadata.json") != Some(&metadata_hash) {
        return Err("checksum manifest does not match release-metadata.json".to_owned());
    }
    validate_metadata(
        &metadata_source.expect("captured installed package release metadata"),
        &package_name,
        &archive_name,
        &manifest,
    )
}

fn package_identity(root: &Path) -> Result<(PathBuf, String, String), String> {
    let metadata = fs::symlink_metadata(root).map_err(|error| {
        format!(
            "cannot inspect installed package {}: {error}",
            root.display()
        )
    })?;
    if !metadata.file_type().is_dir() {
        return Err(format!(
            "installed package must be a directory: {}",
            root.display()
        ));
    }
    let root = fs::canonicalize(root).map_err(|error| {
        format!(
            "cannot resolve installed package {}: {error}",
            root.display()
        )
    })?;
    let package_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "installed package directory name is not UTF-8".to_owned())?
        .to_owned();
    let archive_name = format!("{package_name}.tar.gz");
    crate::release_archive::parse_archive_name(&archive_name)?;
    Ok((root, package_name, archive_name))
}

fn hash_regular_file(path: &Path, capture: bool) -> Result<(String, Option<Vec<u8>>, u64), String> {
    crate::release_archive::regular_file(
        path,
        "installed package member",
        Some(if capture {
            MAX_CONTROL_BYTES
        } else {
            MAX_ARCHIVE_BYTES
        }),
    )?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).map_err(|error| {
        format!(
            "cannot open installed package member {}: {error}",
            path.display()
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        format!(
            "cannot inspect installed package member {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "installed package member must be a regular file: {}",
            path.display()
        ));
    }
    let maximum = if capture {
        MAX_CONTROL_BYTES
    } else {
        MAX_ARCHIVE_BYTES
    };
    if metadata.len() > maximum {
        return Err(format!(
            "installed package member is too large: {}",
            path.display()
        ));
    }
    let result =
        hash_reader(&mut (&mut file).take(metadata.len() + 1), capture).map_err(|error| {
            format!(
                "cannot hash installed package member {}: {error}",
                path.display()
            )
        })?;
    if result.2 != metadata.len() {
        return Err(format!(
            "installed package member changed while being verified: {}",
            path.display()
        ));
    }
    Ok(result)
}

fn hash_reader(
    reader: &mut impl Read,
    capture: bool,
) -> std::io::Result<(String, Option<Vec<u8>>, u64)> {
    let mut digest = Sha256::new();
    let mut source = capture.then(Vec::new);
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        size += count as u64;
        if capture && size > MAX_CONTROL_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "installed package control file grew beyond its size limit",
            ));
        }
        if let Some(source) = source.as_mut() {
            source.extend_from_slice(&buffer[..count]);
        }
    }
    let hash = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok((hash, source, size))
}

fn manifest_directories<'a>(paths: impl Iterator<Item = &'a String>) -> BTreeSet<String> {
    let mut directories = BTreeSet::new();
    for path in paths {
        let parts = path.split('/').collect::<Vec<_>>();
        for length in 1..parts.len() {
            directories.insert(parts[..length].join("/"));
        }
    }
    directories
}

fn set_error(label: &str, expected: &BTreeSet<String>, actual: &BTreeSet<String>) -> String {
    let missing = expected.difference(actual).cloned().collect::<Vec<_>>();
    let unexpected = actual.difference(expected).cloned().collect::<Vec<_>>();
    let mut details = Vec::new();
    if !missing.is_empty() {
        details.push(format!("missing {}", missing.join(", ")));
    }
    if !unexpected.is_empty() {
        details.push(format!("unexpected {}", unexpected.join(", ")));
    }
    format!("{label}: {}", details.join("; "))
}
