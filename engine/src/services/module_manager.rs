use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::models::module::{ModuleInfo, ModuleManifest};

const MODULE_MANIFEST_SCHEMA_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024;
const MAX_MODULES: usize = 128;
const MAX_CAPABILITIES: usize = 32;

#[derive(Clone, Debug)]
pub struct ModuleManager {
    inner: Arc<RwLock<HashMap<String, ModuleInfo>>>,
}

impl ModuleManager {
    pub fn from_env() -> Result<Self, ModuleManagerError> {
        if let Some(configured) = std::env::var_os("CYANREX_MODULES_DIR") {
            if configured.is_empty() {
                return Err(ModuleManagerError::new(
                    "CYANREX_MODULES_DIR must not be empty",
                ));
            }
            return Self::discover(PathBuf::from(configured));
        }

        let runtime_path = std::env::current_dir()
            .map_err(|error| ModuleManagerError::new(format!("read current directory: {error}")))?
            .join("modules");
        if runtime_path.is_dir() {
            return Self::discover(runtime_path);
        }

        let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("modules");
        Self::discover(source_path)
    }

    pub fn discover(root: impl AsRef<Path>) -> Result<Self, ModuleManagerError> {
        let root = root.as_ref();
        let entries = fs::read_dir(root).map_err(|error| {
            ModuleManagerError::new(format!("read module catalog {}: {error}", root.display()))
        })?;
        let mut directories = entries
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| ModuleManagerError::new(format!("read module catalog: {error}")))?;
        directories.sort_by_key(|entry| entry.file_name());

        let mut modules = HashMap::new();
        for entry in directories {
            let file_type = entry.file_type().map_err(|error| {
                ModuleManagerError::new(format!("inspect {}: {error}", entry.path().display()))
            })?;
            if !file_type.is_dir() {
                continue;
            }
            let manifest_path = entry.path().join("module.json");
            let metadata = match fs::symlink_metadata(&manifest_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(ModuleManagerError::new(format!(
                        "inspect module manifest {}: {error}",
                        manifest_path.display()
                    )))
                }
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(ModuleManagerError::new(format!(
                    "module manifest must be a regular file: {}",
                    manifest_path.display()
                )));
            }
            if metadata.len() > MAX_MANIFEST_BYTES {
                return Err(ModuleManagerError::new(format!(
                    "module manifest exceeds {MAX_MANIFEST_BYTES} bytes: {}",
                    manifest_path.display()
                )));
            }

            let manifest: ModuleManifest =
                serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| {
                    ModuleManagerError::new(format!(
                        "read module manifest {}: {error}",
                        manifest_path.display()
                    ))
                })?)
                .map_err(|error| {
                    ModuleManagerError::new(format!(
                        "parse module manifest {}: {error}",
                        manifest_path.display()
                    ))
                })?;
            let directory_name = entry.file_name().to_string_lossy().into_owned();
            let module = validate_manifest(manifest, &directory_name, &manifest_path)?;
            if modules.insert(module.name.clone(), module).is_some() {
                return Err(ModuleManagerError::new(format!(
                    "duplicate module name in catalog: {directory_name}"
                )));
            }
            if modules.len() > MAX_MODULES {
                return Err(ModuleManagerError::new(format!(
                    "module catalog exceeds {MAX_MODULES} entries"
                )));
            }
        }

        Ok(Self {
            inner: Arc::new(RwLock::new(modules)),
        })
    }

    pub fn list(&self) -> Vec<ModuleInfo> {
        let guard = self.inner.read().expect("module manager lock poisoned");
        let mut modules = guard.values().cloned().collect::<Vec<_>>();
        modules.sort_by(|left, right| left.name.cmp(&right.name));
        modules
    }

    pub fn start(&self, name: &str) -> Result<ModuleInfo, ModuleManagerError> {
        self.set_status(name, "running")
    }

    pub fn stop(&self, name: &str) -> Result<ModuleInfo, ModuleManagerError> {
        self.set_status(name, "stopped")
    }

    fn set_status(&self, name: &str, status: &str) -> Result<ModuleInfo, ModuleManagerError> {
        let normalized = name.trim();
        let mut guard = self.inner.write().expect("module manager lock poisoned");
        let module = guard.get_mut(normalized).ok_or_else(|| {
            ModuleManagerError::new(format!(
                "unknown module '{normalized}'; inspect GET /modules for the catalog"
            ))
        })?;
        module.status = status.to_string();
        Ok(module.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleManagerError {
    message: String,
}

impl ModuleManagerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ModuleManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ModuleManagerError {}

fn validate_manifest(
    manifest: ModuleManifest,
    directory_name: &str,
    manifest_path: &Path,
) -> Result<ModuleInfo, ModuleManagerError> {
    let context = || format!("module manifest {}", manifest_path.display());
    if manifest
        .schema
        .as_deref()
        .is_some_and(|schema| schema != "../module.schema.json")
    {
        return Err(ModuleManagerError::new(format!(
            "{} references an unsupported $schema",
            context()
        )));
    }
    if manifest.schema_version != MODULE_MANIFEST_SCHEMA_VERSION {
        return Err(ModuleManagerError::new(format!(
            "{} has unsupported schema_version {}; expected {MODULE_MANIFEST_SCHEMA_VERSION}",
            context(),
            manifest.schema_version
        )));
    }
    if !valid_identifier(&manifest.name, 128) {
        return Err(ModuleManagerError::new(format!(
            "{} has an invalid module name",
            context()
        )));
    }
    if manifest.name != directory_name {
        return Err(ModuleManagerError::new(format!(
            "{} name '{}' must match directory name '{directory_name}'",
            context(),
            manifest.name
        )));
    }
    if !valid_semantic_version(&manifest.version) {
        return Err(ModuleManagerError::new(format!(
            "{} version '{}' must be a semantic version",
            context(),
            manifest.version
        )));
    }
    let description = manifest.description.trim().to_string();
    if description.is_empty() || description.len() > 512 {
        return Err(ModuleManagerError::new(format!(
            "{} description must contain 1 to 512 bytes",
            context()
        )));
    }
    if manifest.capabilities.len() > MAX_CAPABILITIES {
        return Err(ModuleManagerError::new(format!(
            "{} declares more than {MAX_CAPABILITIES} capabilities",
            context()
        )));
    }
    let mut seen = HashSet::new();
    for capability in &manifest.capabilities {
        if !valid_identifier(capability, 64) {
            return Err(ModuleManagerError::new(format!(
                "{} has invalid capability '{capability}'",
                context()
            )));
        }
        if !seen.insert(capability) {
            return Err(ModuleManagerError::new(format!(
                "{} repeats capability '{capability}'",
                context()
            )));
        }
    }

    Ok(ModuleInfo {
        name: manifest.name,
        status: "stopped".to_string(),
        version: manifest.version,
        description,
        capabilities: manifest.capabilities,
    })
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= max_len
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_semantic_version(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 || !value.is_ascii() {
        return false;
    }
    let core_end = value.find(['-', '+']).unwrap_or(value.len());
    let core = &value[..core_end];
    let suffix = &value[core_end..];
    let parts = core.split('.').collect::<Vec<_>>();
    let valid_core = parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        });
    valid_core && valid_semantic_suffix(suffix)
}

fn valid_semantic_suffix(suffix: &str) -> bool {
    if suffix.is_empty() {
        return true;
    }
    if let Some(build) = suffix.strip_prefix('+') {
        return valid_semantic_identifiers(build, false);
    }
    let Some(prerelease_and_build) = suffix.strip_prefix('-') else {
        return false;
    };
    let (prerelease, build) = prerelease_and_build
        .split_once('+')
        .map_or((prerelease_and_build, None), |(left, right)| {
            (left, Some(right))
        });
    valid_semantic_identifiers(prerelease, true)
        && build.is_none_or(|value| valid_semantic_identifiers(value, false))
}

fn valid_semantic_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(reject_numeric_leading_zero
                    && identifier.len() > 1
                    && identifier.starts_with('0')
                    && identifier.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::ModuleManager;

    #[test]
    fn discovers_versioned_manifests_and_manages_only_known_modules() {
        let catalog = TestCatalog::new();
        catalog.add_manifest(
            "module-network",
            r#"{
              "schema_version": 1,
              "name": "module-network",
              "version": "0.3.0",
              "description": "Publishes normalized network events.",
              "capabilities": ["network.events", "event.publish"]
            }"#,
        );
        fs::create_dir_all(catalog.path().join("module-protocol")).unwrap();

        let manager = ModuleManager::discover(catalog.path()).expect("catalog should load");
        let modules = manager.list();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "module-network");
        assert_eq!(modules[0].version, "0.3.0");
        assert_eq!(modules[0].status, "stopped");
        assert_eq!(modules[0].capabilities, ["network.events", "event.publish"]);

        let running = manager
            .start(" module-network ")
            .expect("known module starts");
        assert_eq!(running.status, "running");
        let stopped = manager.stop("module-network").expect("known module stops");
        assert_eq!(stopped.status, "stopped");

        let error = manager.start("module-unknown").unwrap_err();
        assert!(error.to_string().contains("unknown module"));
        assert_eq!(manager.list().len(), 1);
    }

    #[test]
    fn rejects_unsupported_or_directory_mismatched_manifests() {
        let unsupported = TestCatalog::new();
        unsupported.add_manifest(
            "module-network",
            r#"{
              "schema_version": 2,
              "name": "module-network",
              "version": "0.3.0",
              "description": "Network module.",
              "capabilities": []
            }"#,
        );
        assert!(ModuleManager::discover(unsupported.path())
            .unwrap_err()
            .to_string()
            .contains("schema_version"));

        let mismatched = TestCatalog::new();
        mismatched.add_manifest(
            "module-network",
            r#"{
              "schema_version": 1,
              "name": "module-ebpf",
              "version": "0.3.0",
              "description": "Wrong directory.",
              "capabilities": []
            }"#,
        );
        assert!(ModuleManager::discover(mismatched.path())
            .unwrap_err()
            .to_string()
            .contains("directory name"));
    }

    #[test]
    fn rejects_missing_catalog_and_invalid_manifest_values() {
        let missing = TestCatalog::new();
        let missing_path = missing.path().join("does-not-exist");
        assert!(ModuleManager::discover(&missing_path).is_err());

        let invalid = TestCatalog::new();
        invalid.add_manifest(
            "module-network",
            r#"{
              "schema_version": 1,
              "name": "module-network",
              "version": "latest",
              "description": "Network module.",
              "capabilities": ["network events"]
            }"#,
        );
        let message = ModuleManager::discover(invalid.path())
            .unwrap_err()
            .to_string();
        assert!(message.contains("semantic version") || message.contains("capability"));

        let invalid_suffix = TestCatalog::new();
        invalid_suffix.add_manifest(
            "module-network",
            r#"{
              "schema_version": 1,
              "name": "module-network",
              "version": "1.0.0-",
              "description": "Network module.",
              "capabilities": []
            }"#,
        );
        assert!(ModuleManager::discover(invalid_suffix.path())
            .unwrap_err()
            .to_string()
            .contains("semantic version"));
    }

    struct TestCatalog {
        root: PathBuf,
    }

    impl TestCatalog {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "cyanrex-module-catalog-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn add_manifest(&self, directory: &str, manifest: &str) {
            let module_path = self.root.join(directory);
            fs::create_dir_all(&module_path).unwrap();
            fs::write(module_path.join("module.json"), manifest).unwrap();
        }
    }

    impl Drop for TestCatalog {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
