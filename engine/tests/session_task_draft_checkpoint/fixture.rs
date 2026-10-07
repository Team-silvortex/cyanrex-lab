use super::*;

pub const FORMAT: &str = "cyanrex.task-draft-checkpoint";
pub const TITLE: &str = "  描述 · e\u{301} 📝  ";
pub const SECRET_TEXT: &str = "private-code-sentinel-993c\r\n\t😀";

pub fn wire(count: usize) -> Value {
    let scope = json!({
        "authority_id": "00000000-0000-0000-0000-000000000001",
        "workspace_id": "00000000-0000-0000-0000-000000000002",
    });
    let payload: Vec<_> = (0..count)
        .map(|_| {
            json!({
                "kind": "text", "filename": "重复.txt", "language": "future.language+v2",
                "artifact": { "workspace": scope.clone(), "artifact_id": Uuid::new_v4().to_string(),
                    "revision_id": Uuid::new_v4().to_string(), "sha256": "a".repeat(64) },
            })
        })
        .collect();
    json!({
        "format": FORMAT, "version": 1,
        "task": { "workspace": scope, "task_id": Uuid::new_v4().to_string() },
        "manifest": { "schema_version": 1, "title": TITLE, "payload": payload },
        "byte_lengths": vec![0; count], "reported_state": "ready",
        "reported_confirmed_artifact_count": 0,
    })
}

pub fn parse(value: &Value) -> Result<SessionDraftPublicationCheckpoint, ContractError> {
    SessionDraftPublicationCheckpoint::parse_json(&serde_json::to_vec(value).unwrap())
}

pub fn raw_at(value: &Value, pointer: &str, raw: &str) -> Vec<u8> {
    let mut value = value.clone();
    *value.pointer_mut(pointer).unwrap() = json!("__CHECKPOINT_RAW_RECORD__");
    serde_json::to_string(&value)
        .unwrap()
        .replace("\"__CHECKPOINT_RAW_RECORD__\"", raw)
        .into_bytes()
}

pub fn duplicate_at(value: &Value, pointer: &str, key: &str, replacement: &Value) -> Vec<u8> {
    // Build raw JSON, not a Value map that would silently discard the duplicate under test.
    let record = serde_json::to_string(value.pointer(pointer).unwrap()).unwrap();
    let duplicate = format!(
        "{{{}:{},{}",
        serde_json::to_string(key).unwrap(),
        serde_json::to_string(replacement).unwrap(),
        &record[1..]
    );
    raw_at(value, pointer, &duplicate)
}

pub struct Fixture {
    pub source: DurableAuthSource,
    pub workspace: SessionTaskContentWorkspace,
    pub root: PathBuf,
    pub token: String,
}

impl Fixture {
    pub async fn ready() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        let scope = WorkspaceRef {
            authority_id: Uuid::new_v4().try_into().unwrap(),
            workspace_id: Uuid::new_v4().try_into().unwrap(),
        };
        let source = DurableAuthSource::new(pool, scope.authority_id);
        let root = std::env::temp_dir().join(format!("cyanrex-checkpoint-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let artifacts = source
            .open_artifact_workspace("checkpoint_artifacts", scope, &root)
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new("checkpoint_tasks", scope, artifacts).unwrap();
        Self {
            source,
            workspace,
            root,
            token: Uuid::new_v4().to_string(),
        }
    }

    pub fn attempt(&self, texts: &[&str]) -> SessionTaskDraftPublication {
        self.attempt_with_labels(TITLE, "重复.txt", "future.language+v2", texts)
    }

    pub fn attempt_with_labels(
        &self,
        title: &str,
        filename: &str,
        language: &str,
        texts: &[&str],
    ) -> SessionTaskDraftPublication {
        let payload: Vec<_> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                json!({
                    "id": format!("local_only_{index}"), "revision": 9007199254740991_u64,
                    "kind": "text", "filename": filename, "language": language, "text": text,
                })
            })
            .collect();
        let plan = TaskDraftPublicationPlan::parse_json(
            &serde_json::to_vec(&json!({
                "format": "cyanrex.task-draft", "version": 1, "title": title, "payload": payload,
            }))
            .unwrap(),
        )
        .unwrap();
        self.source
            .prepare_task_draft_publication(&self.token, self.workspace.clone(), plan)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the exact empty directory created above; the closed pool prevents publication.
        fs::remove_dir(&self.root).unwrap();
    }
}
