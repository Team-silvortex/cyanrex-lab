pub(super) use super::{
    content_fixture::{manifest, task_id},
    publication_http::{call, error, headers, request, unread, wire, PATH},
    retained_fixture::{artifact_id, revision_id, snapshot, token_hash, within, Fixture},
};
use cyanrex_engine::platform_http::{build_platform_workspace_router, PlatformWorkspaceHttpState};

pub(super) struct Prepared<'a> {
    base: super::retained_fixture::Prepared<'a>,
}
impl<'a> std::ops::Deref for Prepared<'a> {
    type Target = super::retained_fixture::Prepared<'a>;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}
impl<'a> Prepared<'a> {
    pub(super) async fn new(f: &'a Fixture) -> Self {
        let base = super::retained_fixture::Prepared::new(f).await;
        for grant in [
            "INSERT ON collaboration_tasks",
            "INSERT ON collaboration_task_outbox",
            "UPDATE(revision,snapshot,manifest) ON collaboration_tasks",
        ] {
            f.sql(&format!(
                "GRANT {} TO {}",
                grant.replace(" ON ", &format!(" ON {}.", f.task)),
                f.role
            ))
            .await;
        }
        // Existing exact-content GET/PUT keep their old row-lock policy. Publication alone
        // does not need this column grant; the permission selector explicitly removes it.
        f.sql(&format!(
            "GRANT UPDATE(artifact_id) ON {}.collaboration_artifacts TO {}",
            f.artifacts, f.role
        ))
        .await;
        Self { base }
    }
    pub(super) async fn router(&self, capacities: [usize; 7]) -> axum::Router {
        let [login, lifecycle, index, content, create, replace, publish] = capacities;
        build_platform_workspace_router(
            PlatformWorkspaceHttpState::with_text_publication_capacities(
                self.storage().await,
                login,
                lifecycle,
                index,
                content,
                create,
                replace,
                publish,
            )
            .unwrap(),
        )
    }
    pub(super) fn finish(self) -> bool {
        self.base.finish()
    }
}

pub(super) async fn task_counts(f: &Fixture, heads: i64, events: i64) {
    use sqlx_core::{query::query, row::Row};
    for (table, expected) in [
        ("collaboration_tasks", heads),
        ("collaboration_task_outbox", events),
    ] {
        let actual: i64 = within(
            query(&format!("SELECT count(*) AS count FROM {}.{table}", f.task))
                .fetch_one(f.owner()),
        )
        .await
        .unwrap()
        .get("count");
        assert_eq!(actual, expected, "{table}");
    }
}
