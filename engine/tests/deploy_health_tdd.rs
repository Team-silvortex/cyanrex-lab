#![cfg(unix)]
//! Synthetic offline packages and real loopback HTTP only; Docker is always a refusing fixture.

#[path = "deploy_health/fixture.rs"]
mod fixture;
#[path = "deploy_health/native.rs"]
mod native;
#[path = "deploy_health/package.rs"]
mod package;
