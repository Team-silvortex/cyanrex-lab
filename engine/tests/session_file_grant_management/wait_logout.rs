use super::{
    fixture::*,
    publisher_fixture::{grant, validity},
    support::*,
    waits::marker_holder,
    *,
};

pub(crate) async fn command_admitted_before_real_logout() {
    let f = Fixture::ready().await;
    let before = f.base.snapshot().await;
    let mut holder = marker_holder(&f).await;
    let holder_pid = backend(&mut holder).await;
    let mut pending = Box::pin(f.base.auth.source.issue_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(1),
        f.target(),
        validity(),
    ));
    let mut early = None;
    let entered =
        tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(&f,holder_pid)=>w};
    let command_pid:Option<i32>=query("SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
        .bind(&f.base.auth.base.schema).bind(holder_pid).fetch_optional(&f.base.auth.base.admin).await.unwrap().map(|r|r.get("pid"));
    let mut logout = Box::pin(f.base.auth.source.logout(&f.base.auth.learner_token));
    let mut early_logout = None;
    let logout_waited = if let Some(command_pid) = command_pid {
        tokio::select! {
            r=&mut logout=>{early_logout=Some(r);false},r=&mut pending,if early.is_none()=>{early=Some(r);false},w=blocked(&f,command_pid)=>w
        }
    } else {
        false
    };
    holder.rollback().await.unwrap();
    // Poll BOTH operations: logout cannot pass the real source fence until issuance commits.
    let (issued, logged_out) = tokio::join!(
        async {
            match early {
                Some(r) => r,
                None => pending.as_mut().await,
            }
        },
        async {
            match early_logout {
                Some(r) => r,
                None => logout.as_mut().await,
            }
        }
    );
    drop(pending);
    drop(logout);
    let after = f.base.snapshot().await;
    let mut expected = before.clone();
    let digest = source_fixture::token_hash(&f.base.auth.learner_token);
    let sessions = &mut expected
        .0
        .iter_mut()
        .find(|(n, _, _)| n == &format!("{}.sessions", f.base.auth.base.schema))
        .unwrap()
        .2;
    let count = sessions.len();
    sessions.retain(|r| {
        serde_json::from_str::<serde_json::Value>(r).unwrap()["token"] != serde_json::json!(digest)
    });
    assert_eq!(sessions.len() + 1, count);
    delta(&f, &expected, &after, 1, false);
    let denied = issue(&f, 2, f.target()).await;
    let saved = direct_get(&f, 1, f.target()).await;
    restored(&f).await;
    f.cleanup().await;
    assert!(entered && logout_waited);
    let record = issued.unwrap();
    assert!(logged_out.is_ok());
    assert_eq!(saved.unwrap(), Some(record));
    assert_eq!(
        denied,
        Err(session_error(SessionCommandError::InvalidSession))
    );
}
