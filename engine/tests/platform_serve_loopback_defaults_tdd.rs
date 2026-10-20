#![cfg(target_os = "linux")]
//! Real capability-reduced CLI rejection/peer probes. No real database or live login success.

#[path = "platform_serve_loopback_defaults/cli.rs"]
mod cli;
#[path = "platform_serve_loopback_defaults/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[path = "platform_serve_loopback_defaults/peer.rs"]
mod peer;
#[path = "platform_serve_loopback_defaults/support.rs"]
mod support;
