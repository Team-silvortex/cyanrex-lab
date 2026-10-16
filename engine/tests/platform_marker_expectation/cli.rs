use super::{
    catalog_fixture::Fixture,
    expectation_helpers::{quote_identifier, Pins},
    marker_fixture::{prepare, snapshot},
    process_helpers::{passed, rejected, Input, Run},
    support::{no_connections, relation, MarkerPins},
};
use serde_json::json;
use std::{fs, path::Path};

async fn run_formats(input: &Input, path2: &Path, path3: &Path) -> [Run; 3] {
    [
        input.spawn(None).finish(false).await,
        input.spawn(Some(path2)).finish(false).await,
        input.spawn(Some(path3)).finish(false).await,
    ]
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_marker_expectation_cli_v3_rejects_original_marker_owner_while_v2_is_diagnostic(
) {
    let (
        healthy,
        namespace_owner,
        marker_owner,
        bad_version,
        restored,
        no_repair,
        unchanged,
        no_backend,
        cleaned,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let auxiliary = fixture.additional_role().await;
            let catalog = Pins::read(fixture).await;
            let markers = MarkerPins::read(fixture).await;
            let input = Input::new(fixture);
            let before = snapshot(fixture).await;
            let mut v2 = input.document.clone();
            v2["format_version"] = json!(2);
            v2["catalog_identity"] = catalog.json();
            let mut v3 = v2.clone();
            v3["format_version"] = json!(3);
            v3["marker_identity"] = markers.json();
            let path2 = input.write_variant("original-catalog-v2.json", &v2);
            let path3 = input.write_variant("original-markers-v3.json", &v3);
            let original2 = fs::read(&path2).unwrap();
            let original3 = fs::read(&path3).unwrap();
            let owner = quote_identifier(&fixture.owner_name().await);

            // v1 observes current catalog/markers, v2 retains catalog identity,
            // and v3 additionally retains all marker relation/owner identities.
            let healthy = run_formats(&input, &path2, &path3).await;
            let mut no_repair = before == snapshot(fixture).await
                && catalog == Pins::read(fixture).await
                && markers == MarkerPins::read(fixture).await;
            fixture
                .sql(&format!(
                    "ALTER SCHEMA {} OWNER TO {auxiliary}",
                    fixture.task
                ))
                .await;
            let changed = snapshot(fixture).await;
            let changed_catalog = Pins::read(fixture).await;
            let namespace_owner = run_formats(&input, &path2, &path3).await;
            no_repair &= changed == snapshot(fixture).await
                && changed_catalog == Pins::read(fixture).await
                && markers == MarkerPins::read(fixture).await;
            fixture
                .sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", fixture.task))
                .await;

            fixture
                .sql(&format!(
                    "ALTER TABLE {} OWNER TO {auxiliary}",
                    relation(fixture, 0)
                ))
                .await;
            let changed = snapshot(fixture).await;
            let changed_pins = MarkerPins::read(fixture).await;
            let marker_owner = run_formats(&input, &path2, &path3).await;
            no_repair &= changed == snapshot(fixture).await
                && changed_pins == MarkerPins::read(fixture).await
                && catalog == Pins::read(fixture).await;
            fixture
                .sql(&format!(
                    "ALTER TABLE {} OWNER TO {owner}",
                    relation(fixture, 0)
                ))
                .await;

            // Identity retention does not waive the real marker body/version check.
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=0 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let changed = snapshot(fixture).await;
            let bad_version = run_formats(&input, &path2, &path3).await;
            no_repair &= changed == snapshot(fixture).await
                && catalog == Pins::read(fixture).await
                && markers == MarkerPins::read(fixture).await;
            fixture
                .sql(&format!(
                    "UPDATE {} SET version=2 WHERE singleton",
                    relation(fixture, 0)
                ))
                .await;
            let restored = run_formats(&input, &path2, &path3).await;
            let unchanged = before == snapshot(fixture).await
                && markers == MarkerPins::read(fixture).await
                && catalog == Pins::read(fixture).await
                && fs::read(&path2).unwrap() == original2
                && fs::read(&path3).unwrap() == original3;
            let no_backend = no_connections(fixture).await;
            let cleaned = input.finish();
            (
                healthy,
                namespace_owner,
                marker_owner,
                bad_version,
                restored,
                no_repair,
                unchanged,
                no_backend,
                cleaned,
            )
        })
    })
    .await;
    // Every direct child, private input directory, and SQL fixture is cleaned first.
    for result in healthy.iter().chain(&restored) {
        passed(result);
    }
    passed(&namespace_owner[0]);
    rejected(&namespace_owner[1]);
    rejected(&namespace_owner[2]);
    passed(&marker_owner[0]);
    passed(&marker_owner[1]);
    rejected(&marker_owner[2]);
    for result in &bad_version {
        rejected(result);
    }
    assert!(no_repair && unchanged && no_backend && cleaned);
}
