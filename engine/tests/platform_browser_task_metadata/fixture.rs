use super::browser_fixture;
pub(super) use super::browser_fixture::{
    counter, exact_session, task_id, token_hash, within, Fixture,
};
use cyanrex_engine::{
    models::collaboration::{TaskContentManifest, TaskSnapshot, TextPayloadBinding},
    services::task_store::TaskContentSnapshot,
};
use serde_json::{json, Value};
use sqlx_core::{query::query, row::Row};

pub(super) const SAVED_TITLE: &str = "Revised collaborative seminar outline";
pub(super) const SAVED_NAMES: [&str; 2] = ["共同教学说明.txt", "세미나자료.txt"];
pub(super) const SAVED_LANGUAGES: [&str; 2] = ["teaching-notes-v2", "future-notes-v9"];
pub(super) const CONFLICT_TITLE: &str = "Unsaved group discussion draft";
pub(super) const CONFLICT_NAME: &str = "未提交说明.txt";
pub(super) const CONFLICT_LANGUAGE: &str = "notes-draft-v3";
pub(super) const ADMIN_TITLE: &str = "Another confirmed collaboration edit";
pub(super) const REVOKED_TITLE: &str = "Revoked-session private unsaved draft";
pub(super) const REVOKED_NAME: &str = "撤销后不得保存.txt";
pub(super) const REVOKED_LANGUAGE: &str = "private-notes-v4";

pub(super) struct Prepared<'a> {
    pub(super) base: browser_fixture::Prepared<'a>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = browser_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = browser_fixture::Prepared::new(f).await;
        // Explicit disposable administration only: reuse read/row-lock grants, adding exactly
        // the existing Task replacement writes. No Task INSERT/DELETE or Artifact business write.
        f.sql(&format!(
            "GRANT UPDATE(revision,snapshot,manifest) ON {}.collaboration_tasks TO {}",
            f.task, f.role
        ))
        .await;
        f.sql(&format!(
            "GRANT INSERT ON {}.collaboration_task_outbox TO {}",
            f.task, f.role
        ))
        .await;
        Self { base }
    }
    pub(super) fn input(&self) -> Value {
        let mut config = self.base.browser_input();
        let values = json!({"saved_title":SAVED_TITLE,"saved_names":SAVED_NAMES,
            "saved_languages":SAVED_LANGUAGES,"conflict_title":CONFLICT_TITLE,
            "conflict_name":CONFLICT_NAME,"conflict_language":CONFLICT_LANGUAGE,
            "admin_title":ADMIN_TITLE,"revoked_title":REVOKED_TITLE,
            "revoked_name":REVOKED_NAME,"revoked_language":REVOKED_LANGUAGE});
        config
            .as_object_mut()
            .unwrap()
            .extend(values.as_object().unwrap().clone());
        config
    }
    pub(super) async fn current(&self) -> TaskContentSnapshot {
        let (source, workspace) = self.base.base.pooled();
        source
            .get_session_content_task(&self.base.base.tokens[0], &workspace, task_id(10))
            .await
            .unwrap()
            .unwrap()
            .snapshot
    }
    pub(super) async fn admin_advance(&self, old: &TaskContentSnapshot) -> TaskContentSnapshot {
        let (source, workspace) = self.base.base.pooled();
        // A legal independently committed command, not a raw revision corruption: exact old/new
        // bytes, current owner, snapshot and matching outbox all pass the existing service.
        let manifest =
            TaskContentManifest::new(ADMIN_TITLE, old.manifest.payload().to_vec()).unwrap();
        source
            .replace_session_content_task(
                &self.base.base.tokens[0],
                &workspace,
                task_id(10),
                old.task.revision,
                manifest,
            )
            .await
            .unwrap()
    }
    pub(super) async fn assert_head_event(&self, expected: &TaskContentSnapshot, events: i64) {
        let f = self.base.base.f;
        assert_eq!(self.current().await, *expected);
        let head = within(
            query(&format!("SELECT owner_id,revision,snapshot,manifest FROM {}.collaboration_tasks WHERE task_id=$1",f.task))
                .bind(task_id(10).as_uuid()).fetch_one(f.owner()),
        ).await.unwrap();
        assert_eq!(
            head.get::<uuid::Uuid, _>("owner_id"),
            expected.task.owner.principal_id.as_uuid()
        );
        assert_eq!(
            head.get::<i64, _>("revision"),
            u64::from(expected.task.revision) as i64
        );
        assert_eq!(
            serde_json::from_str::<TaskSnapshot>(&head.get::<String, _>("snapshot")).unwrap(),
            expected.task
        );
        assert_eq!(
            TaskContentManifest::parse_json(head.get::<String, _>("manifest").as_bytes()).unwrap(),
            expected.manifest
        );
        let rows=within(query(&format!("SELECT actor_id,revision,event_type,snapshot,manifest FROM {}.collaboration_task_outbox WHERE task_id=$1 ORDER BY revision",f.task)).bind(task_id(10).as_uuid()).fetch_all(f.owner())).await.unwrap();
        assert_eq!(rows.len() as i64, events);
        for (index, event) in rows.iter().enumerate() {
            assert_eq!(event.get::<i64, _>("revision"), index as i64 + 1);
            assert_eq!(
                event.get::<uuid::Uuid, _>("actor_id"),
                expected.task.owner.principal_id.as_uuid()
            );
            assert_eq!(
                event.get::<String, _>("event_type"),
                if index == 0 {
                    "cyanrex.task.created"
                } else {
                    "cyanrex.task.content_updated"
                }
            );
        }
        let event = rows.last().unwrap();
        assert_eq!(
            event.get::<String, _>("snapshot"),
            head.get::<String, _>("snapshot")
        );
        assert_eq!(
            event.get::<String, _>("manifest"),
            head.get::<String, _>("manifest")
        );
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}

pub(super) fn saved_manifest(old: &TaskContentSnapshot) -> TaskContentManifest {
    TaskContentManifest::new(
        SAVED_TITLE,
        old.manifest
            .payload()
            .iter()
            .enumerate()
            .map(|(index, item)| {
                TextPayloadBinding::new(
                    item.artifact().clone(),
                    SAVED_NAMES[index],
                    SAVED_LANGUAGES[index],
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

pub(super) async fn immutable_rows(f: &Fixture) -> Vec<(String, String)> {
    let mut rows = browser_fixture::resources(f).await;
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let name = format!("{}.{table}", f.task);
        rows.retain(|(table, _)| table != &name);
        let digest:String=within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {name} t WHERE task_id<>$1")).bind(task_id(10).as_uuid()).fetch_one(f.owner())).await.unwrap().get("digest");
        rows.push((name, digest));
    }
    rows.sort();
    rows
}

pub(super) async fn sessions(f: &Fixture) -> String {
    within(query(&format!("SELECT md5(coalesce(string_agg(to_jsonb(t)::text,'|' ORDER BY to_jsonb(t)::text),'')) AS digest FROM {}.sessions t",f.source)).fetch_one(f.owner())).await.unwrap().get("digest")
}

pub(super) async fn revoke(f: &Fixture, digest: &str) {
    let deleted = within(
        query(&format!("DELETE FROM {}.sessions WHERE token=$1", f.source))
            .bind(digest)
            .execute(f.owner()),
    )
    .await
    .unwrap();
    assert_eq!(deleted.rows_affected(), 1);
}

pub(super) async fn exact_profile(f: &Fixture) {
    for (schema, table, insert) in [
        (&f.task, "collaboration_tasks", false),
        (&f.task, "collaboration_task_outbox", true),
        (&f.artifacts, "collaboration_artifacts", false),
        (&f.artifacts, "collaboration_artifact_revisions", false),
        (&f.artifacts, "collaboration_artifact_outbox", false),
    ] {
        let name = format!("{schema}.{table}");
        let row=within(query("SELECT has_table_privilege($1,$2,'INSERT') AS i,has_table_privilege($1,$2,'UPDATE') AS u,has_table_privilege($1,$2,'DELETE') AS d,has_table_privilege($1,$2,'TRIGGER') AS t").bind(&f.role).bind(&name).fetch_one(f.owner())).await.unwrap();
        assert_eq!(row.get::<bool, _>("i"), insert);
        assert!(!row.get::<bool, _>("u") && !row.get::<bool, _>("d") && !row.get::<bool, _>("t"));
    }
    // Artifact head UPDATE(artifact_id) is the pre-existing read row-lock permission, not a new
    // business grant. The three Task metadata columns are the only newly granted UPDATE columns.
    for column in ["revision", "snapshot", "manifest"] {
        let allowed: bool = within(
            query("SELECT has_column_privilege($1,$2,$3,'UPDATE') AS allowed")
                .bind(&f.role)
                .bind(format!("{}.collaboration_tasks", f.task))
                .bind(column)
                .fetch_one(f.owner()),
        )
        .await
        .unwrap()
        .get("allowed");
        assert!(allowed);
    }
}
