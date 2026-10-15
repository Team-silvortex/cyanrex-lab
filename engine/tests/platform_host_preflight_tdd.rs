#![cfg(unix)]

#[path = "platform_host_preflight/behavior.rs"]
mod behavior;
#[path = "platform_host_preflight/boundaries.rs"]
mod boundaries;
#[path = "platform_host_preflight/config.rs"]
mod config;
#[path = "platform_host_preflight/fixture.rs"]
mod fixture;
#[path = "platform_host_preflight/namespaces.rs"]
mod namespaces;
#[path = "platform_host_preflight/roles.rs"]
mod roles;
