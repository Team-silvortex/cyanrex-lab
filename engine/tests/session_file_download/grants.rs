use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_requires_exact_active_recipient_grant_without_role_or_publisher_bypass()
{
    checked(Fixture::ready().await, |f| Box::pin(async move {
        for token in [&auth(f).learner_token,&auth(f).manager_token] {
            assert_eq!(read_as(f,token,&f.workspace,&f.request()).await,Err(SessionFileDownloadError::PermissionDenied),
                "publisher ownership or teacher/deployment-manager status is not recipient authority");
        }
        assert_eq!(read(f,&request(f,99)).await,Err(SessionFileDownloadError::PermissionDenied));
        let actor=f.sharing.actor();let target=f.sharing.recipient;let file=f.sharing.expected_file.clone();
        let foreign=FileAccountRef::new(PrincipalRef { authority_id:authority(),principal_id:id() },id());
        for (seed,publisher,recipient,validity) in [
            (2,actor,target,FileGrantValidity::new(1,2).unwrap()),
            (3,actor,target,FileGrantValidity::new(MAX_FILE_GRANT_UNIX_SECONDS-1,MAX_FILE_GRANT_UNIX_SECONDS).unwrap()),
            (4,actor,auth_account(f),grant_fixture::validity()),
            (5,actor,FileAccountRef::new(target.principal(),id()),grant_fixture::validity()),
            (6,actor,FileAccountRef::new(PrincipalRef{principal_id:id(),..target.principal()},target.account_id()),grant_fixture::validity()),
            (7,FileAccountRef::new(actor.principal(),id()),target,grant_fixture::validity()),
            (8,foreign,target,grant_fixture::validity()),
            (9,actor,target,grant_fixture::validity()),
        ] { issue(f,seed,foreign,publisher,recipient,file.clone(),validity).await; }
        let revoked=f.sharing.store.revoke(sharing_fixture::recipient_grant(9),foreign,sharing_fixture::revision(1)).await.unwrap();
        assert_eq!(revoked.grant().state(),FileGrantState::Revoked);
        for seed in 2..=9 {assert_eq!(read(f,&request(f,seed)).await,Err(SessionFileDownloadError::PermissionDenied),"grant {seed}");}
        // A self-share is allowed ONLY with its own explicit active exact recipient grant.
        issue(f,10,foreign,actor,actor,file.clone(),grant_fixture::validity()).await;
        assert_eq!(read_as(f,&auth(f).learner_token,&f.workspace,&request(f,10)).await.unwrap(),f.sharing.base.manifest);
        issue(f,11,foreign,actor,target,file,grant_fixture::validity()).await;
        assert_eq!(read(f,&request(f,11)).await.unwrap(),f.sharing.base.manifest,
            "issuer is separately stored attribution, not an implicit current-user or teacher gate");
        // Neither publisher permission nor the old publisher's current identity/source presence
        // is a dependency of this independently active recipient grant.
        membership(f,actor.principal().principal_id,MembershipStatus::Suspended).await;healthy(f).await;
        retire(f,&auth(f).learner).await;healthy(f).await;
        query(&format!("DELETE FROM {}.users WHERE account_id=$1",auth(f).base.schema))
            .bind(actor.account_id().as_uuid()).execute(&auth(f).base.admin).await.unwrap();
        healthy(f).await;assert_eq!(read(f,&request(f,11)).await.unwrap(),f.sharing.base.manifest);drain(f).await;
    })).await;
}

fn auth_account(f: &Fixture) -> FileAccountRef {
    FileAccountRef::new(
        auth(f).manager.principal.reference,
        auth(f).manager.account_id,
    )
}
