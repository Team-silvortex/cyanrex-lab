use super::super::{DurableAuthError, DurableAuthSource};
use crate::sqlx_compat::{query, Row};

#[allow(dead_code)]
#[path = "otp_sql_support.rs"]
mod support;
use support::*;

#[path = "otp_consumption_sql_support.rs"]
mod consumption_support;
use consumption_support::*;
#[path = "otp_consumption_sql_behavior.rs"]
mod behavior;
#[path = "otp_consumption_sql_faults.rs"]
mod faults;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_otp_consumption_independent_sources_cannot_both_use_one_code() {
    let f = Fixture::ready().await;
    let separate = f.separate_pool("otp-consumption-independent-source").await;
    let independent = f.independent_source(separate.clone());
    let name = username();
    let code = f.code();
    let (first, second) = tokio::join!(
        f.source.login(&name, PASSWORD, &code),
        independent.login(&name, PASSWORD, &code),
    );
    let outcomes = [first.map(|_| ()), second.map(|_| ())];
    let sessions = f.count_sessions().await;
    separate.close().await;
    f.cleanup().await;
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Err(DurableAuthError::InvalidCredentials))
            .count(),
        1
    );
    assert_eq!(sessions, 1, "one code must publish exactly one Session");
}
