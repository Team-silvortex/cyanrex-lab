#![cfg(unix)]
//! Explicit current-Session lifecycle transport, never live Engine authentication or issuance.

#[path = "platform_session_http/behavior.rs"]
mod behavior;
#[path = "platform_session_http/capacity_sql.rs"]
mod capacity_sql;
#[path = "platform_session_http/faults.rs"]
mod faults;
#[path = "platform_session_http/fixture.rs"]
mod fixture;
#[path = "platform_session_http/transport.rs"]
mod transport;
#[path = "platform_session_http/waits.rs"]
mod waits;
