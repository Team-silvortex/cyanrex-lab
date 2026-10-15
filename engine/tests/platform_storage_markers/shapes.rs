use super::{
    fixture::{prepare, within, Fixture},
    support::*,
};
use cyanrex_engine::platform_host::PlatformHostPreflightError as Error;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_markers_reject_unsafe_relations_and_metadata_column_shapes() {
    let (results, restorations, traps, marker_only) = Fixture::checked(|fixture| Box::pin(async move {
        prepare(fixture).await;
        let original = catalog(fixture).await;
        let mut results = Vec::new();
        let mut restorations = Vec::new();
        let mut traps = Vec::new();
        let owner = fixture.owner_name().await.replace('"', "\"\"");
        for marker in markers(fixture) {
            for (change, restore) in [
                ("ENABLE ROW LEVEL SECURITY", "DISABLE ROW LEVEL SECURITY"),
                ("FORCE ROW LEVEL SECURITY", "NO FORCE ROW LEVEL SECURITY"),
                ("SET UNLOGGED", "SET LOGGED"),
                ("ALTER COLUMN version DROP NOT NULL", "ALTER COLUMN version SET NOT NULL"),
                ("ALTER COLUMN version TYPE bigint", "ALTER COLUMN version TYPE integer"),
            ] {
                fixture.sql(&format!("ALTER TABLE {} {change}", marker.name())).await;
                results.push(checked_inspect(fixture, &original).await);
                fixture.sql(&format!("ALTER TABLE {} {restore}", marker.name())).await;
                restorations.push(checked_inspect(fixture, &original).await);
            }
            // Test inheritance on a disposable copy: dropping a child does not reliably clear
            // PostgreSQL's conservative relhassubclass flag, even after VACUUM.
            let inherited_original = "retained_inherited_marker";
            fixture.sql(&format!("ALTER TABLE {} RENAME TO {inherited_original}", marker.name())).await;
            fixture.sql(&format!("CREATE TABLE {} (LIKE {}.{inherited_original} INCLUDING ALL)", marker.name(), marker.schema)).await;
            fixture.sql(&format!("INSERT INTO {} SELECT * FROM {}.{inherited_original}", marker.name(), marker.schema)).await;
            grant_marker(fixture, &marker).await;
            restorations.push(checked_inspect(fixture, &original).await);
            let child = format!("{}.marker_inheritance_child", marker.schema);
            fixture.sql(&format!("CREATE TABLE {child} () INHERITS ({})", marker.name())).await;
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("DROP TABLE {child}")).await;
            fixture.sql(&format!("DROP TABLE {}", marker.name())).await;
            fixture.sql(&format!("ALTER TABLE {}.{inherited_original} RENAME TO {}", marker.schema, marker.table)).await;
            restorations.push(checked_inspect(fixture, &original).await);

            let generated_original = "retained_generated_marker";
            fixture.sql(&format!("ALTER TABLE {} RENAME TO {generated_original}", marker.name())).await;
            let scope_definitions = if marker.scope_columns == 2 { ",authority_id uuid NOT NULL,workspace_id uuid NOT NULL" }
                else if marker.scope_columns == 1 { ",authority_id uuid NOT NULL" } else { "" };
            fixture.sql(&format!("CREATE TABLE {} (singleton boolean PRIMARY KEY CHECK (singleton),
                version integer GENERATED ALWAYS AS ({}) STORED NOT NULL{scope_definitions})", marker.name(), marker.version)).await;
            let row_columns = if marker.scope_columns == 2 { "singleton,authority_id,workspace_id" }
                else if marker.scope_columns == 1 { "singleton,authority_id" } else { "singleton" };
            fixture.sql(&format!("INSERT INTO {} ({row_columns}) SELECT {row_columns} FROM {}.{generated_original}", marker.name(), marker.schema)).await;
            grant_marker(fixture, &marker).await;
            // The replacement has exactly the expected live column count and valid values;
            // this fault isolates a generated required field rather than an extra-column failure.
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("DROP TABLE {}", marker.name())).await;
            fixture.sql(&format!("ALTER TABLE {}.{generated_original} RENAME TO {}", marker.schema, marker.table)).await;
            restorations.push(checked_inspect(fixture, &original).await);

            fixture.sql(&format!("GRANT TRIGGER ON {} TO {}", marker.name(), fixture.role)).await;
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("REVOKE TRIGGER ON {} FROM {}", marker.name(), fixture.role)).await;
            restorations.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("ALTER TABLE {} OWNER TO {}", marker.name(), fixture.role)).await;
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("ALTER TABLE {} OWNER TO \"{owner}\"", marker.name())).await;
            // PostgreSQL transfers the old owner's explicit ACL when ownership changes back.
            // Remove those retained privileges so the next fault is tested in isolation.
            fixture.sql(&format!("REVOKE ALL ON {} FROM {}", marker.name(), fixture.role)).await;
            grant_marker(fixture, &marker).await;
            restorations.push(checked_inspect(fixture, &original).await);

            let key: String = within(query("SELECT conname::text AS name FROM pg_catalog.pg_constraint
                WHERE conrelid=pg_catalog.to_regclass($1) AND contype='p'")
                .bind(marker.name()).fetch_one(fixture.owner())).await
                .unwrap_or_else(|_| panic!("original marker key identity unavailable")).get("name");
            fixture.sql(&format!("ALTER TABLE {} DROP CONSTRAINT {key}", marker.name())).await;
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("ALTER TABLE {} ADD CONSTRAINT {key} PRIMARY KEY (singleton)", marker.name())).await;
            restorations.push(checked_inspect(fixture, &original).await);

            let retained = format!("retained_{}", marker.table);
            fixture.sql(&format!("ALTER TABLE {} RENAME TO {retained}", marker.name())).await;
            fixture.sql(&format!("CREATE FUNCTION {}.marker_read_trap() RETURNS integer LANGUAGE plpgsql AS $$ BEGIN
                RAISE EXCEPTION 'synthetic forbidden marker view evaluation' USING ERRCODE='P0001'; END $$", marker.schema)).await;
            let rest = if marker.scope_columns == 2 { ",authority_id,workspace_id" }
                else if marker.scope_columns == 1 { ",authority_id" } else { "" };
            fixture.sql(&format!("CREATE VIEW {} AS SELECT singleton,{}.marker_read_trap() AS version{rest} FROM {}.{retained}", marker.name(), marker.schema, marker.schema)).await;
            grant_marker(fixture, &marker).await;
            let control = within(query(&format!("SELECT version FROM {}", marker.name())).execute(fixture.owner())).await;
            traps.push(control.as_ref().err().and_then(|error| error.as_database_error())
                .is_some_and(|error| error.code().as_deref()==Some("P0001")));
            results.push(checked_inspect(fixture, &original).await);
            fixture.sql(&format!("DROP VIEW {}", marker.name())).await;
            fixture.sql(&format!("DROP FUNCTION {}.marker_read_trap()", marker.schema)).await;
            fixture.sql(&format!("ALTER TABLE {}.{retained} RENAME TO {}", marker.schema, marker.table)).await;
            restorations.push(checked_inspect(fixture, &original).await);
        }
        // This API deliberately does not promise complete business-storage compatibility.
        fixture.sql(&format!("DROP TABLE {}.collaboration_task_outbox", fixture.task)).await;
        fixture.sql(&format!("DROP TABLE {}.collaboration_artifact_outbox", fixture.artifacts)).await;
        let marker_only = checked_inspect(fixture, &original).await;
        (results, restorations, traps, marker_only)
    })).await;
    assert_eq!(results.len(), 55);
    for (result, unchanged) in results {
        assert_eq!(result, Err(Error::UnsupportedStorageMarker));
        assert!(unchanged);
    }
    assert_eq!(traps.len(), 5);
    assert!(
        traps.into_iter().all(|raised| raised),
        "every synthetic dangerous view must actually raise when evaluated"
    );
    assert_eq!(restorations.len(), 60);
    for (index, (result, unchanged)) in restorations.into_iter().enumerate() {
        assert_eq!(result, Ok(()), "restoration scenario {index}");
        assert!(unchanged);
    }
    assert_eq!(marker_only.0, Ok(()));
    assert!(marker_only.1);
}
