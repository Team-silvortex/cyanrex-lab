use super::{fixture::*, publisher_fixture::validity, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_manager_is_explicit_principal_generation_not_teacher_or_deployment_role(
) {
    let f = Fixture::ready().await;
    let old = issue(&f, 1, f.target()).await.unwrap();
    for token in [&f.base.auth.manager_token] {
        assert_eq!(
            issue_as(&f, token, &f.management, 2, f.target(), validity()).await,
            Err(SessionFilePublisherGrantError::PermissionDenied)
        );
        assert_eq!(
            revoke_as(&f, token, &f.management, 1, revision(1)).await,
            Err(SessionFilePublisherGrantError::PermissionDenied)
        );
    }
    let wrong_generation = FileAccountRef::new(f.manager().principal(), f.target().account_id());
    let wrong_principal = FileAccountRef::new(f.target().principal(), f.manager().account_id());
    for wrong in [wrong_generation, wrong_principal] {
        let configuration = f
            .base
            .auth
            .source
            .file_grant_management_workspace(f.base.workspace.clone(), wrong)
            .unwrap();
        assert_eq!(
            issue_as(
                &f,
                &f.base.auth.learner_token,
                &configuration,
                2,
                f.target(),
                validity()
            )
            .await,
            Err(SessionFilePublisherGrantError::PermissionDenied)
        );
        assert_eq!(
            revoke_as(
                &f,
                &f.base.auth.learner_token,
                &configuration,
                1,
                revision(1)
            )
            .await,
            Err(SessionFilePublisherGrantError::PermissionDenied)
        );
    }
    // A deliberate trusted host configuration selecting the teacher is distinct from its roles;
    // surviving old configuration clones also remain usable, not implicitly revoked by this new one.
    let teacher = f
        .base
        .auth
        .source
        .file_grant_management_workspace(f.base.workspace.clone(), f.target())
        .unwrap();
    let issued = issue_as(
        &f,
        &f.base.auth.manager_token,
        &teacher,
        2,
        f.manager(),
        validity(),
    )
    .await
    .unwrap();
    assert_eq!(issued.issuer(), f.target());
    let clone = f.management.clone();
    let revoked = revoke_as(&f, &f.base.auth.learner_token, &clone, 1, revision(1))
        .await
        .unwrap();
    assert_eq!(revoked.issuer(), old.issuer());
    assert_eq!(
        read(&f, &f.base.auth.learner_token, 2).await.unwrap(),
        Some(issued)
    );
    f.cleanup().await;
}
