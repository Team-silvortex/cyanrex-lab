#![cfg(target_os = "linux")]

#[allow(dead_code)]
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[path = "platform_storage_check_defaults/peer.rs"]
mod peer;
#[path = "platform_storage_check_defaults/rejections.rs"]
mod rejections;
#[path = "platform_storage_check_defaults/support.rs"]
mod support;
