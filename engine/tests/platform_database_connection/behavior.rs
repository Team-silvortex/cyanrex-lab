use super::{
    catalog_fixture::{within, Fixture},
    helpers::{connector, identity},
};
use sqlx_core::connection::Connection;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_connection_opens_fresh_verified_read_only_connections_without_mutation()
{
    let (created, opened, different_backends, policy, same_original, unchanged, tables) =
        Fixture::checked(|fixture| {
            Box::pin(async move {
                let before = fixture.snapshot().await;
                let original = fixture
                    .observe()
                    .await
                    .unwrap_or_else(|_| panic!("owned original catalog observation unavailable"));
                let created = connector(fixture).await;
                let created_ok = created.is_ok();
                let mut opened = 0;
                let mut different_backends = false;
                let mut policy = false;
                let mut same_original = false;
                if let Ok(connector) = created {
                    let first = within(connector.connect()).await;
                    let second = within(connector.connect()).await;
                    opened = usize::from(first.is_ok()) + usize::from(second.is_ok());
                    match (first, second) {
                        (Ok(mut first), Ok(mut second)) => {
                            let a = identity(fixture, &original, &mut first).await;
                            let b = identity(fixture, &original, &mut second).await;
                            different_backends = a.0 != b.0 && a.0 > 0 && b.0 > 0;
                            policy = a.1 && b.1;
                            same_original = a.2 && b.2;
                            within(first.close()).await.unwrap();
                            within(second.close()).await.unwrap();
                        }
                        (first, second) => {
                            if let Ok(connection) = first {
                                within(connection.close()).await.unwrap();
                            }
                            if let Ok(connection) = second {
                                within(connection.close()).await.unwrap();
                            }
                        }
                    }
                }
                let after = fixture.snapshot().await;
                (
                    created_ok,
                    opened,
                    different_backends,
                    policy,
                    same_original,
                    before == after,
                    after.tables,
                )
            })
        })
        .await;
    // Behavior assertions run only after checked cleanup removed the owned database,
    // administrator/runtime connections, login role and any auxiliary roles.
    assert!(
        created,
        "the connector must retain an explicit original target"
    );
    assert_eq!(opened, 2);
    assert!(
        different_backends,
        "each call must open a fresh physical connection"
    );
    assert!(
        policy,
        "startup settings and runtime role must match exactly"
    );
    assert!(
        same_original,
        "both connections must revalidate the original identity"
    );
    assert!(
        unchanged,
        "connecting must not install or mutate catalog storage"
    );
    assert_eq!(tables, 0);
}
