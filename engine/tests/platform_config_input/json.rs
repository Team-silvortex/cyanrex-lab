use super::fixture::{passed, rejected, Fixture, CANARY};
use serde_json::{json, Value};
use std::{fs, io, net::TcpListener, path::Path};

pub(super) fn document(artifact_root: &Path, port: u16) -> Value {
    json!({
        "format_version": 1,
        "origin": "https://platform.example",
        "authority_id": "00000000-0000-0000-0000-000000001069",
        "workspace_id": "00000000-0000-0000-0000-00000000106a",
        "namespaces": {"source": "source_private", "task": "task_private", "artifacts": "artifacts_private"},
        "database": {
            "database": "platform_db", "role": "platform_runtime", "system_identifier": 123,
            "database_oid": 456, "transport": {"kind": "loopback", "address": "127.0.0.1"},
            "port": port, "password": CANARY
        },
        "session_limits": {"per_account": 4, "total": 16},
        "artifact_root": artifact_root
    })
}

#[test]
fn platform_config_cli_loads_private_explicit_json_without_network_or_ambient_authority() {
    let (
        safe_ids,
        outputs,
        untouched,
        no_connection,
        no_ambient_reads,
        watch_positive,
        no_artifact,
    ) = Fixture::checked(|fixture| {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let root = fixture.path("artifact-root-must-not-be-opened");
        let bytes =
            serde_json::to_vec(&document(&root, listener.local_addr().unwrap().port())).unwrap();
        let pgpass = fixture.write("ambient.pgpass", CANARY.as_bytes(), 0o600);
        let service = fixture.write("ambient.service", CANARY.as_bytes(), 0o600);
        let dotenv = fixture.write(".env", CANARY.as_bytes(), 0o600);
        let mut watch = super::fixture::Watch::new(&[&pgpass, &service, &dotenv]);
        let mut outputs = Vec::new();
        let mut untouched = true;
        let mut no_connection = true;
        for (index, mode) in [0o400, 0o600].into_iter().enumerate() {
            let input = fixture.write(&format!("valid-{index}.json"), &bytes, mode);
            outputs.push(fixture.run(&fixture.arguments(&input), true));
            untouched &= fs::read(&input).unwrap() == bytes;
            no_connection &= match listener.accept() {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => true,
                Ok((socket, _)) => {
                    drop(socket);
                    false
                }
                Err(_) => false,
            };
        }
        let no_ambient_reads = !watch.activity();
        for path in [&pgpass, &service, &dotenv] {
            fs::read(path).unwrap();
        }
        let watch_positive = watch.activity();
        (
            fixture.identity.safe_ids(),
            outputs,
            untouched,
            no_connection,
            no_ambient_reads,
            watch_positive,
            !root.exists(),
        )
    });
    for output in &outputs {
        if safe_ids {
            passed(output);
        } else {
            rejected(output, 1);
        }
    }
    assert!(untouched && no_connection && no_ambient_reads && watch_positive && no_artifact);
}

#[test]
fn platform_config_cli_rejects_ambiguous_or_invalid_json_without_echoing_secrets() {
    let (outputs, unchanged) = Fixture::checked(|fixture| {
        let original = document(&fixture.path("unused-artifacts"), 5432);
        let mut cases = Vec::<Vec<u8>>::new();
        cases.extend([
            b"".to_vec(),
            b"{".to_vec(),
            b"[]".to_vec(),
            b"null".to_vec(),
            format!("{{\"password\":\"{CANARY}\"").into_bytes(),
            b"{\"a\":1,}".to_vec(),
        ]);
        for (field, value) in [
            ("format_version", json!(0)),
            ("format_version", json!(2)),
            ("origin", json!("http://192.0.2.1")),
            (
                "authority_id",
                json!("00000000-0000-0000-0000-000000000000"),
            ),
            ("workspace_id", json!("not-a-uuid")),
            ("artifact_root", json!("relative")),
        ] {
            let mut changed = original.clone();
            changed[field] = value;
            cases.push(serde_json::to_vec(&changed).unwrap());
        }
        for (section, field, value) in [
            ("namespaces", "source", json!("public")),
            ("namespaces", "task", json!("source_private")),
            ("database", "system_identifier", json!(0)),
            ("database", "database_oid", json!(0)),
            ("database", "port", json!(0)),
            ("database", "password", json!("synthetic\u{0}invalid")),
            ("session_limits", "per_account", json!(0)),
            ("session_limits", "total", json!(1)),
        ] {
            let mut changed = original.clone();
            changed[section][field] = value;
            cases.push(serde_json::to_vec(&changed).unwrap());
        }
        for pointer in [
            "",
            "/namespaces",
            "/database",
            "/database/transport",
            "/session_limits",
        ] {
            let mut unknown = original.clone();
            unknown
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(CANARY.into(), json!(CANARY));
            cases.push(serde_json::to_vec(&unknown).unwrap());
        }
        let compact = serde_json::to_string(&original).unwrap();
        cases.push(format!("{{\"format_version\":1,{}", &compact[1..]).into_bytes());
        cases.push(
            compact
                .replace("\"port\":5432", "\"port\":5432,\"port\":5432")
                .into_bytes(),
        );
        cases.push(format!("{compact} {compact}").into_bytes());
        let mut outputs = Vec::new();
        let mut unchanged = true;
        for (index, bytes) in cases.iter().enumerate() {
            let input = fixture.write(&format!("bad-{index}.json"), bytes, 0o600);
            outputs.push(fixture.run(&fixture.arguments(&input), false));
            unchanged &= fs::read(&input).unwrap() == *bytes;
        }
        (outputs, unchanged)
    });
    assert!(unchanged);
    assert!(outputs.len() >= 25);
    let generic = &outputs[0].output.stderr;
    for output in &outputs {
        rejected(output, 1);
        assert_eq!(&output.output.stderr, generic);
    }
}
