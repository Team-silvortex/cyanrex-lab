#![cfg(target_os = "linux")]
//! Real private roots and loopback failure peers; never PostgreSQL authentication or live storage.

#[path = "platform_storage_open_defaults/environment.rs"]
mod environment;
#[path = "platform_storage_open_defaults/fixture.rs"]
mod fixture;
#[path = "platform_storage_open_defaults/peer.rs"]
mod peer;
#[path = "platform_storage_open_defaults/rejections.rs"]
mod rejections;
