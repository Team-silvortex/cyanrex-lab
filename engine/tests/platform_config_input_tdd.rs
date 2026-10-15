#![cfg(target_os = "linux")]

#[path = "platform_config_input/files.rs"]
mod files;
#[path = "platform_config_input/fixture.rs"]
mod fixture;
#[path = "platform_config_input/json.rs"]
mod json;
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[path = "platform_config_input/process.rs"]
mod process;
