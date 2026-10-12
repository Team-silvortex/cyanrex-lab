use crate::{fixture::Fixture, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_requires_current_exact_recipient_grant_without_publisher_or_teacher_bypass(
) {
    checked(Fixture::ready(1).await, |f| {
        Box::pin(async move {
            for token in [&auth(f).learner_token, &auth(f).manager_token] {
                assert_eq!(
                    read_as(f, &auth(f).source, token, &f.workspace, &f.request()).await,
                    Err(denied())
                );
            }
            assert_eq!(read(f, &request(f, 99)).await, Err(denied()));
            let actor = f.sharing.actor();
            let target = f.sharing.recipient;
            let file = f.sharing.expected_file.clone();
            for (seed, publisher, recipient, v) in [
                (2, actor, target, FileGrantValidity::new(1, 2).unwrap()),
                (
                    3,
                    actor,
                    target,
                    FileGrantValidity::new(
                        MAX_FILE_GRANT_UNIX_SECONDS - 1,
                        MAX_FILE_GRANT_UNIX_SECONDS,
                    )
                    .unwrap(),
                ),
                (4, actor, actor, grant_fixture::validity()),
                (
                    5,
                    actor,
                    FileAccountRef::new(target.principal(), id()),
                    grant_fixture::validity(),
                ),
                (
                    6,
                    FileAccountRef::new(actor.principal(), id()),
                    target,
                    grant_fixture::validity(),
                ),
                (7, actor, target, grant_fixture::validity()),
            ] {
                issue(f, seed, publisher, recipient, file.clone(), v).await;
            }
            f.sharing
                .store
                .revoke(
                    sharing_fixture::recipient_grant(7),
                    actor,
                    sharing_fixture::revision(1),
                )
                .await
                .unwrap();
            for seed in 2..=7 {
                assert_eq!(
                    read(f, &request(f, seed)).await,
                    Err(denied()),
                    "grant {seed}"
                );
            }
            assert_eq!(
                read_as(
                    f,
                    &auth(f).source,
                    &auth(f).learner_token,
                    &f.workspace,
                    &request(f, 4)
                )
                .await
                .unwrap(),
                expected(file.clone(), 0, 17),
                "self-recipient bytes require a separate explicit exact active self grant"
            );
            let unrelated_issuer = FileAccountRef::new(PrincipalRef {
                authority_id: authority(), principal_id: id(),
            }, id());
            assert_ne!(unrelated_issuer, actor);
            let before = f.sharing.snapshot().await;
            let independent = f.sharing.store.issue(sharing_fixture::recipient_grant(8),
                unrelated_issuer, file.clone(), actor, target, grant_fixture::validity()).await.unwrap();
            let after = f.sharing.snapshot().await;
            let exact = FileRecipientGrant::new(sharing_fixture::recipient_grant(8),
                sharing_fixture::revision(1), file.clone(), actor, target,
                grant_fixture::validity(), FileGrantState::Active).unwrap();
            sharing_fixture::recipient_delta(&f.sharing.schema, &before, &after, 8, false);
            sharing_fixture::assert_record_rows(&after, &f.sharing.schema, &exact,
                unrelated_issuer, unrelated_issuer);
            assert_eq!(independent.grant(), &exact);
            assert_eq!(independent.issuer(), unrelated_issuer);
            assert_eq!(read(f, &request(f, 8)).await.unwrap(), expected(file.clone(), 0, 17),
                "separately stored trusted issuer attribution is not an implicit current-issuer or issuer-equals-publisher read gate");
            for (field, value) in [
                ("file_id", serde_json::json!(Uuid::new_v4())),
                ("version_id", serde_json::json!(Uuid::new_v4())),
                ("byte_length", serde_json::json!(18)),
                ("sha256", serde_json::json!("b".repeat(64))),
            ] {
                let mut r = f.request();
                let mut raw = serde_json::to_value(&r.download.expected_file).unwrap();
                raw[field] = value;
                r.download.expected_file = serde_json::from_value(raw).unwrap();
                assert_eq!(read(f, &r).await, Err(denied()), "exact coordinate {field}");
            }
            let mut r = f.request();
            r.download.upload_id = publication_fixture::upload(99);
            assert_eq!(read(f, &r).await, Err(denied()));
            // Damage current storage after health control. Denied/future/revoked actors must still
            // get their exact authorization denial, with no byte DTO (not a corrupt-prefix response).
            let root = f.sharing.base.root.clone();
            let moved = f.sharing.base.directory.join("withheld-root");
            std::fs::rename(&root, &moved).unwrap();
            f.sharing.base.root = moved;
            for seed in [2, 3, 7, 99] {
                assert_eq!(read(f, &request(f, seed)).await, Err(denied()));
            }
            assert_eq!(
                read_as(
                    f,
                    &auth(f).source,
                    &auth(f).manager_token,
                    &f.workspace,
                    &f.request()
                )
                .await,
                Err(denied())
            );
            std::fs::rename(&f.sharing.base.root, &root).unwrap();
            f.sharing.base.root = root;
            healthy(f).await;
            drain(f).await;
        })
    })
    .await;
}
