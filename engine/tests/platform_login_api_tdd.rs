#![cfg(unix)]
//! Default API compile contract only; real authorization requires the explicit SQL suite.
use cyanrex_engine::{
    models::collaboration::LegacyUsername,
    platform_host::{PlatformLoginConnector, PlatformStorageConnector},
    services::auth_service::durable_source::{DurableAuthError, DurableLogin},
};

async fn login_result(
    connector: &PlatformLoginConnector,
    username: &LegacyUsername,
) -> Result<DurableLogin, DurableAuthError> {
    connector
        .login(username, "synthetic-password", "000000")
        .await
}

#[test]
fn bound_login_consumes_one_original_storage_binding_and_keeps_secret_return_typed() {
    let _: fn(PlatformStorageConnector) -> PlatformLoginConnector = PlatformLoginConnector::new;
    let _: for<'a> fn(&'a PlatformLoginConnector) -> &'a str = PlatformLoginConnector::origin;
    fn shared<T: Send + Sync>() {}
    shared::<PlatformLoginConnector>();
    let _ = login_result;
}
