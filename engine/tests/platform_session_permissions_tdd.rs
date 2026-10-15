#![cfg(unix)]
//! A real restricted login role, not a privileged pool or mocked Session source.

#[path = "platform_session_permissions/behavior.rs"]
mod behavior;
#[path = "platform_session_permissions/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_session_http/fixture.rs"]
mod http_fixture;
#[path = "platform_session_permissions/rejections.rs"]
mod rejections;
#[path = "platform_session_permissions/shapes.rs"]
mod shapes;
#[path = "platform_session_permissions/waits.rs"]
mod waits;
