use super::{
    catalog_fixture::{within, Fixture},
    fixture::token_hash,
};
use cyanrex_engine::{
    models::collaboration::*,
    services::collaboration_identity_store::{CollaborationIdentityStore, LegacyAccessPolicy},
};
use sqlx_core::query::query;

async fn template(f: &Fixture, schema: &str, text: &str, delimiter: &str) {
    path(f, schema).await;
    for statement in text
        .split(delimiter)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        within(query(statement).execute(f.owner())).await.unwrap();
    }
}

pub(super) async fn path(f: &Fixture, schema: &str) {
    // The fixture owner pool has exactly one connection; no concurrent store uses this path.
    within(
        query("SELECT pg_catalog.set_config('search_path',$1,false)")
            .bind(schema)
            .execute(f.owner()),
    )
    .await
    .unwrap();
}

pub(super) async fn prepare(f: &Fixture, tokens: &[String; 3]) -> [PrincipalRef; 2] {
    for (text, delimiter) in [
        (
            include_str!("../../migrations/0001_auth_users_sessions.sql"),
            ";",
        ),
        (
            include_str!("../../migrations/0010_durable_auth_source.sql"),
            "-- cyanrex-statement",
        ),
        (
            include_str!("../../migrations/0015_durable_otp_consumption.sql"),
            ";",
        ),
    ] {
        template(f, &f.source, text, delimiter).await;
    }
    let scope = f.config().scope();
    within(
        query("INSERT INTO collaboration_auth_source_schema VALUES(TRUE,2,$1)")
            .bind(scope.authority_id.as_uuid())
            .execute(f.owner()),
    )
    .await
    .unwrap();
    let registry = CollaborationIdentityStore::new(f.owner().clone());
    registry.install_schema().await.unwrap();
    registry.provision_legacy_workspace(scope).await.unwrap();
    registry.install_access_schema().await.unwrap();
    let mut principals = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let name = format!("index-owner-{index}");
        let account: LegacyAccountId = uuid::Uuid::from_u128(8300 + index as u128)
            .try_into()
            .unwrap();
        // Committed synthetic Sessions only: neither credential enrollment nor login is claimed.
        within(
            query(
                "INSERT INTO users(username,password_salt,password_hash,totp_secret,account_id)
            VALUES($1,'synthetic-salt','synthetic-noncredential','synthetic-nonenrollment',$2)",
            )
            .bind(&name)
            .bind(account.as_uuid())
            .execute(f.owner()),
        )
        .await
        .unwrap();
        within(
            query(
                "INSERT INTO sessions(token,username,account_id,expires_at)
            VALUES($1,$2,$3,clock_timestamp()+interval '12 hours')",
            )
            .bind(token_hash(token))
            .bind(&name)
            .bind(account.as_uuid())
            .execute(f.owner()),
        )
        .await
        .unwrap();
        if index < 2 {
            let identity = registry
                .bind_legacy_account(scope, &name.parse().unwrap(), account)
                .await
                .unwrap();
            let principal = identity.principal.reference;
            registry
                .replace_legacy_access(
                    None,
                    &LegacyAccessPolicy {
                        membership: Membership {
                            workspace: scope,
                            principal_id: principal.principal_id,
                            role_refs: vec!["cyanrex.teaching.teacher".parse().unwrap()],
                            status: MembershipStatus::Active,
                        },
                        deployment_granted: index == 0,
                    },
                )
                .await
                .unwrap();
            principals.push(principal);
        }
    }
    registry.upgrade_policy_audit_schema().await.unwrap();
    registry.upgrade_identity_audit_schema().await.unwrap();
    template(
        f,
        &f.task,
        include_str!("../../migrations/0014_collaboration_task_content.sql"),
        ";",
    )
    .await;
    within(
        query("INSERT INTO collaboration_task_schema VALUES(TRUE,3,$1,$2)")
            .bind(scope.authority_id.as_uuid())
            .bind(scope.workspace_id.as_uuid())
            .execute(f.owner()),
    )
    .await
    .unwrap();
    template(
        f,
        &f.artifacts,
        include_str!("../../migrations/0012_collaboration_artifacts.sql"),
        ";",
    )
    .await;
    within(
        query("INSERT INTO collaboration_artifact_schema VALUES(TRUE,1,$1,$2)")
            .bind(scope.authority_id.as_uuid())
            .bind(scope.workspace_id.as_uuid())
            .execute(f.owner()),
    )
    .await
    .unwrap();
    grants(f).await;
    path(f, &f.source).await;
    [principals[0], principals[1]]
}

async fn grants(f: &Fixture) {
    // Explicit fixture provisioning only. The connector must not grant or override role policy.
    f.sql(&format!(
        "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=off",
        f.role, f.database
    ))
    .await;
    let old = within(f.runtime().acquire()).await.unwrap();
    within(old.close()).await.unwrap();
    for (schema, table, update) in [
        (&f.source, "collaboration_auth_source_schema", "singleton"),
        (&f.source, "collaboration_identity_schema", "singleton"),
        (&f.source, "collaboration_access_schema", "singleton"),
        (&f.source, "collaboration_authorities", "authority_id"),
        (&f.source, "collaboration_workspaces", "workspace_id"),
        (&f.source, "collaboration_legacy_workspaces", "workspace_id"),
        (&f.source, "collaboration_principals", "principal_id"),
        (&f.source, "collaboration_legacy_identities", "principal_id"),
        (&f.source, "collaboration_memberships", "principal_id"),
        (&f.source, "collaboration_deployment_grants", "principal_id"),
        (&f.source, "collaboration_policy_audit", "sequence"),
        (&f.source, "collaboration_identity_audit", "sequence"),
        (&f.task, "collaboration_task_schema", "singleton"),
        (&f.task, "collaboration_tasks", "task_id"),
        (&f.task, "collaboration_task_outbox", "event_id"),
    ] {
        f.sql(&format!(
            "GRANT SELECT,UPDATE({update}) ON {schema}.{table} TO {}",
            f.role
        ))
        .await;
    }
    for grant in [
        format!(
            "SELECT(username,account_id),UPDATE(username) ON {}.users",
            f.source
        ),
        format!(
            "SELECT(token,username,account_id,expires_at),UPDATE(token) ON {}.sessions",
            f.source
        ),
        format!("SELECT ON {}.collaboration_artifact_schema", f.artifacts),
    ] {
        f.sql(&format!("GRANT {grant} TO {}", f.role)).await;
    }
}
