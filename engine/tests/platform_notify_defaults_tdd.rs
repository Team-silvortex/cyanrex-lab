#![cfg(target_os = "linux")]
//! Capability-reduced opt-in notification probes; no service installation or live database.
#[path = "platform_notify_defaults/cli.rs"]
mod cli;
#[allow(dead_code)]
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[allow(dead_code)]
#[path = "platform_serve_loopback_defaults/support.rs"]
mod legacy;
#[allow(dead_code)]
#[path = "platform_notify_defaults/receiver.rs"]
mod receiver;
#[path = "platform_notify_defaults/support.rs"]
mod support;
