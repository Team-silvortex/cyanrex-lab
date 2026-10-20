use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_storage_open_never_adopts_replaced_catalog_or_marker_identities() {
    let (rejected, positive_controls, unchanged, restored, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                prepare(f).await;
                let auxiliary = f.additional_role().await;
                let owner = quote_identifier(&f.owner_name().await);
                let root = Root::new();
                let catalog = Pins::read(f).await;
                let markers = MarkerPins::read(f).await;
                let before = snapshot(f).await;
                let mut rejected = true;
                let mut positive_controls = true;
                let mut unchanged = true;
                // There is deliberately no successful factory call before any substitution. Every
                // declaration comes from the original independently read administrator identity tuple.
                for index in 0..3 {
                    let schema = [&f.source, &f.task, &f.artifacts][index].clone();
                    f.sql(&format!("ALTER SCHEMA {schema} OWNER TO {auxiliary}"))
                        .await;
                    let changed = snapshot(f).await;
                    positive_controls &= current(f).await.is_ok();
                    rejected &= open(f, &root, catalog, markers).await.is_err();
                    unchanged &= changed == snapshot(f).await;
                    f.sql(&format!("ALTER SCHEMA {schema} OWNER TO {owner}"))
                        .await;

                    replace_namespace(f, index).await;
                    let changed = snapshot(f).await;
                    positive_controls &= current(f).await.is_ok();
                    rejected &= open(f, &root, catalog, markers).await.is_err();
                    unchanged &= changed == snapshot(f).await;
                    restore_namespace(f, index).await;
                }
                for index in 0..5 {
                    replace_marker(f, index).await;
                    let changed = snapshot(f).await;
                    positive_controls &= current(f).await.is_ok();
                    rejected &= open(f, &root, catalog, markers).await.is_err();
                    unchanged &= changed == snapshot(f).await;
                    restore_marker(f, index).await;

                    f.sql(&format!(
                        "ALTER TABLE {} OWNER TO {auxiliary}",
                        relation(f, index)
                    ))
                    .await;
                    let changed = snapshot(f).await;
                    positive_controls &= current(f).await.is_ok();
                    rejected &= open(f, &root, catalog, markers).await.is_err();
                    unchanged &= changed == snapshot(f).await;
                    f.sql(&format!(
                        "ALTER TABLE {} OWNER TO {owner}",
                        relation(f, index)
                    ))
                    .await;
                }
                unchanged &= before == snapshot(f).await
                    && catalog == Pins::read(f).await
                    && markers == MarkerPins::read(f).await;
                let restored = match open(f, &root, catalog, markers).await {
                    Ok(binding) => usable(f, &binding).await,
                    Err(_) => false,
                };
                let no_backend = no_connections(f).await;
                let cleaned = root.finish();
                (
                    rejected,
                    positive_controls,
                    unchanged,
                    restored,
                    no_backend,
                    cleaned,
                )
            })
        })
        .await;
    assert!(rejected && positive_controls && unchanged && restored && no_backend && cleaned);
}
