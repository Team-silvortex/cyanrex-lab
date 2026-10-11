use super::fixture::*;
use super::sql_fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_real_task_row_wait_then_expiry_discards_the_page() {
    for catalogue in [false, true] {
        let f = Fixture::ready().await;
        let definitions = values();
        let task = create_at(
            &f,
            10,
            catalogue.then_some(&definitions[0]),
            &f.auth.learner_token,
            manifest("Expiry target", vec![]),
        )
        .await;
        let workspace = if catalogue {
            configured(&f, &definitions)
        } else {
            index(&f)
        };
        assert_eq!(
            list(&f, &workspace, &f.auth.learner_token, None, None)
                .await
                .unwrap()
                .entries,
            vec![entry(&task)]
        );
        let mut holder = lock_task(&f, task.task.reference).await;
        let blocker = backend(&mut holder).await;
        expire_soon(&f).await;
        let before = snapshot(&f).await;
        let mut pending = Box::pin(f.auth.source.list_session_content_tasks(
            &f.auth.learner_token,
            &workspace,
            None,
            None,
        ));
        let mut early = None;
        let waited = tokio::select! {
            result = &mut pending => { early = Some(result); false },
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        let expired = waited && wait_expired(&f).await;
        holder.rollback().await.unwrap();
        let result = match early {
            Some(result) => result,
            None => pending.as_mut().await,
        };
        drop(pending);
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        assert!(
            waited && expired,
            "listing must demonstrably wait on the owned task row before PostgreSQL expiry"
        );
        assert_eq!(
            result,
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(
            snapshot(&f).await,
            before,
            "expired pending metadata never creates rows or touches files"
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_cancelled_real_row_wait_preserves_rows_files_and_pool() {
    for catalogue in [false, true] {
        let f = Fixture::ready().await;
        let definitions = values();
        let task = create_at(
            &f,
            10,
            catalogue.then_some(&definitions[1]),
            &f.auth.learner_token,
            manifest("Cancellation target", vec![]),
        )
        .await;
        let workspace = if catalogue {
            configured(&f, &definitions)
        } else {
            index(&f)
        };
        let expected = list(&f, &workspace, &f.auth.learner_token, None, None)
            .await
            .unwrap();
        assert_eq!(expected.entries, vec![entry(&task)]);
        let before = snapshot(&f).await;
        let mut holder = lock_task(&f, task.task.reference).await;
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(f.auth.source.list_session_content_tasks(
            &f.auth.learner_token,
            &workspace,
            None,
            None,
        ));
        let waited = tokio::select! {
            _ = &mut pending => false,
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        // Drop while the exact reader is blocked; release its blocker only after cancellation.
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        assert!(
            waited,
            "cancellation must occur at a real owned-row wait, not an already finished request"
        );
        assert_eq!(snapshot(&f).await, before);
        assert_eq!(
            list(&f, &workspace, &f.auth.learner_token, None, None)
                .await
                .unwrap(),
            expected
        );
        f.cleanup().await;
    }
}
