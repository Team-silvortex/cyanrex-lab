use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn write_report(report: &Value, output: &Path) -> Result<PathBuf, String> {
    let destination = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve evidence output: {error}"))?
            .join(output)
    };
    let parent = destination
        .parent()
        .ok_or_else(|| format!("evidence output has no parent: {}", destination.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create evidence output directory {}: {error}",
            parent.display()
        )
    })?;
    let name = destination
        .file_name()
        .ok_or_else(|| format!("invalid evidence output path: {}", destination.display()))?
        .to_string_lossy();
    let temporary = parent.join(format!(".{name}.tmp-{}", Uuid::new_v4().simple()));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("cannot create temporary evidence output: {error}"))?;
        serde_json::to_writer_pretty(&mut file, report)
            .map_err(|error| format!("cannot serialize live kernel evidence: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("cannot write live kernel evidence: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync live kernel evidence: {error}"))?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644))
                .map_err(|error| format!("cannot set evidence permissions: {error}"))?;
        }
        fs::hard_link(&temporary, &destination).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                format!(
                    "live kernel evidence output already exists: {}",
                    destination.display()
                )
            } else {
                format!(
                    "cannot publish live kernel evidence {}: {error}",
                    destination.display()
                )
            }
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result?;
    Ok(destination)
}
