use super::{
    catalog_fixture::Fixture,
    marker_fixture::{prepare, snapshot},
    process_helpers::{passed, rejected, Input},
    support::{no_connections, quote_identifier, Pins},
};
use serde_json::json;
use std::fs;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_catalog_expectation_cli_v2_rejects_original_owner_drift_while_v1_remains_diagnostic(
) {
    let (
        wrong_fields,
        pinned_rejection,
        diagnostic,
        restored,
        unchanged,
        no_repair,
        no_backend,
        input_unchanged,
    ) = Fixture::checked(|fixture| {
        Box::pin(async move {
            prepare(fixture).await;
            let auxiliary = fixture.additional_role().await;
            let pins = Pins::read(fixture).await;
            let input = Input::new(fixture);
            let before = snapshot(fixture).await;
            let mut v2 = input.document.clone();
            v2["format_version"] = json!(2);
            v2["catalog_identity"] = pins.json();
            let path = input.write_variant("original-catalog-v2.json", &v2);
            let original_bytes = fs::read(&path).unwrap();
            let mut wrong_fields = Vec::new();
            for index in 0..8 {
                let mut wrong = v2.clone();
                wrong["catalog_identity"] = pins.wrong(index).json();
                let wrong_path =
                    input.write_variant(&format!("wrong-catalog-{index}.json"), &wrong);
                wrong_fields.push(input.spawn(Some(&wrong_path)).finish(false).await);
            }
            let owner = quote_identifier(&fixture.owner_name().await);
            // Drift happens before the very first check-storage invocation. No
            // freshly read role observation may become the v2 declaration.
            fixture
                .sql(&format!(
                    "ALTER SCHEMA {} OWNER TO {auxiliary}",
                    fixture.task
                ))
                .await;
            let changed = snapshot(fixture).await;
            let pinned_rejection = input.spawn(Some(&path)).finish(false).await;
            let mut no_repair = changed == snapshot(fixture).await;
            let diagnostic = input.spawn(None).finish(false).await;
            no_repair &= changed == snapshot(fixture).await;
            fixture
                .sql(&format!("ALTER SCHEMA {} OWNER TO {owner}", fixture.task))
                .await;
            let restored = input.spawn(Some(&path)).finish(false).await;
            let unchanged = before == snapshot(fixture).await
                && Pins::read(fixture).await == pins
                && fs::read(&path).unwrap() == original_bytes;
            let no_backend = no_connections(fixture).await;
            let input_unchanged = input.finish();
            (
                wrong_fields,
                pinned_rejection,
                diagnostic,
                restored,
                unchanged,
                no_repair,
                no_backend,
                input_unchanged,
            )
        })
    })
    .await;
    // The direct children, helper-owned input/root and SQL fixtures are cleaned first.
    assert_eq!(wrong_fields.len(), 8);
    for result in wrong_fields {
        rejected(&result);
    }
    rejected(&pinned_rejection);
    passed(&diagnostic);
    passed(&restored);
    assert!(unchanged && no_repair && no_backend && input_unchanged);
}
