#![cfg(target_os = "linux")]
//! Real capless CLI refusal and synthetic PostgreSQL startup; no live SQL or login success.
#[allow(dead_code)]
#[path = "platform_serve_loopback_tls_defaults/certificates.rs"]
mod certificates;
#[path = "platform_serve_loopback_tls_defaults/cli.rs"]
mod cli;
#[path = "platform_serve_loopback_tls_defaults/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[allow(dead_code)]
#[path = "platform_serve_loopback_defaults/support.rs"]
mod legacy;
#[path = "platform_serve_loopback_tls_defaults/peer.rs"]
mod peer;
#[path = "platform_serve_loopback_tls_defaults/support.rs"]
mod support;
