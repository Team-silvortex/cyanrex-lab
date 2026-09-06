use std::{
    io,
    path::{Path, PathBuf},
};

pub(super) struct CompilerWorkspace {
    path: PathBuf,
}

impl CompilerWorkspace {
    pub(super) fn create(owner_username: &str) -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "cyanrex-compiler-{}-{}",
            super::owner_namespace(owner_username),
            uuid::Uuid::new_v4(),
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        // Create without an await so cancellation cannot occur before the cleanup guard owns it.
        builder.create(&path)?;
        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CompilerWorkspace {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(path = %self.path.display(), "failed to clean compiler workspace: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_workspace_is_private_and_removed_on_drop() {
        let path = {
            let workspace = CompilerWorkspace::create("alice").unwrap();
            let path = workspace.path().to_owned();
            assert!(!path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("alice"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
            std::fs::write(path.join("program.c"), "private source").unwrap();
            path
        };
        assert!(
            !path.exists(),
            "compiler workspace survived drop: {}",
            path.display()
        );
    }

    #[tokio::test]
    async fn compiler_workspace_is_removed_when_operation_is_cancelled() {
        let (ready, receiver) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let workspace = CompilerWorkspace::create("cancelled-student").unwrap();
            std::fs::write(workspace.path().join("program.c"), "private source").unwrap();
            ready.send(workspace.path().to_owned()).unwrap();
            std::future::pending::<()>().await;
            drop(workspace);
        });
        let path = receiver.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            !path.exists(),
            "cancelled compiler left source at {}",
            path.display()
        );
    }
}
