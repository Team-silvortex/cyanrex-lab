use super::EbpfLoader;

#[tokio::test]
async fn compiler_check_cache_is_scoped_to_owner() {
    let loader = EbpfLoader::default();
    let code = "int cyanrex_owner_scope(void) { return 0; }";
    assert!(
        !loader
            .check_for_user_with_cache_status("alice", code, &[])
            .await
            .1
    );
    assert!(
        loader
            .check_for_user_with_cache_status("alice", code, &[])
            .await
            .1
    );
    assert!(
        !loader
            .check_for_user_with_cache_status("bob", code, &[])
            .await
            .1
    );
    assert!(!loader.check_with_cache_status(code, &[]).await.1);
}

#[tokio::test]
async fn compiler_completion_cache_is_scoped_to_owner_and_cursor() {
    let loader = EbpfLoader::default();
    let code = "int cyanrex_owner_scope(void) { return 0; }";
    assert!(
        !loader
            .complete_for_user_with_cache_status("alice", code, 1, 5, &[])
            .await
            .1
    );
    assert!(
        loader
            .complete_for_user_with_cache_status("alice", code, 1, 5, &[])
            .await
            .1
    );
    assert!(
        !loader
            .complete_for_user_with_cache_status("bob", code, 1, 5, &[])
            .await
            .1
    );
    assert!(
        !loader
            .complete_for_user_with_cache_status("alice", code, 1, 6, &[])
            .await
            .1
    );
    assert!(!loader.complete_with_cache_status(code, 1, 5, &[]).await.1);
}
