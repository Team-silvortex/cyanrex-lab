#[test]
fn executor_rejects_ambiguous_paths() {
    assert!(RunnerCompileExecutorConfig::new(
        PathBuf::from("clang"),
        std::env::temp_dir().join("cyanrex-test")
    )
    .is_err());
    assert!(RunnerCompileExecutorConfig::new(
        PathBuf::from("/bin/sh"),
        std::env::temp_dir().join("cyanrex-test")
    )
    .is_err());
}

#[tokio::test]
async fn capped_reader_drains_but_bounds_output() {
    let data = vec![b'x'; 20];
    let capture = read_capped(data.as_slice(), 8).await.unwrap();
    assert_eq!(capture.text, "xxxxxxxx");
    assert!(capture.truncated);
}

#[test]
fn workspaces_are_private_and_removed() {
    let root = std::env::temp_dir().join(format!("cyanrex-executor-test-{}", Uuid::new_v4()));
    let path = {
        let workspace = CompileWorkspace::create(&root).unwrap();
        workspace.path.clone()
    };
    assert!(!path.exists());
    fs::remove_dir(root).unwrap();
}

#[test]
fn source_policy_allows_system_headers_and_rejects_file_reads() {
    assert!(validate_compile_source("#include <linux/bpf.h>\nint x;").is_ok());
    assert!(validate_compile_source("#include \"/run/secrets/token\"\n").is_err());
    assert!(validate_compile_source("#define H <stdio.h>\n#include H\n").is_err());
    assert!(validate_compile_source("#include <../../etc/passwd>\n").is_err());
    assert!(validate_compile_source("#if __has_include(\"/etc/passwd\")\n#endif\n").is_err());
    assert!(validate_compile_source("#inc\\\nlude \"/etc/passwd\"\n").is_err());
    assert!(validate_compile_source("asm(\".inc\" \"bin /etc/passwd\");\n").is_err());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn installed_clang_produces_a_bounded_report_and_leaves_no_job_files() {
    let compiler = PathBuf::from("/usr/bin/clang");
    if !compiler.is_file() {
        return;
    }
    let root = std::env::temp_dir().join(format!("cyanrex-clang-test-{}", Uuid::new_v4()));
    let config = RunnerCompileExecutorConfig::new(compiler, root.clone()).unwrap();
    let report = compile_source(
        &config,
        "int lesson(void) { return 0; }\n",
        Duration::from_secs(10),
    )
    .await
    .unwrap();
    assert!(!report.timed_out);
    if report.success {
        assert!(report.object_bytes.is_some_and(|size| size > 0));
        assert!(report.object_sha256.is_some());
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    fs::remove_dir(root).unwrap();
}
