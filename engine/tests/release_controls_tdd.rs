#![cfg(unix)]
//! Real native CLI subprocesses with synthetic files only; no Docker, SSH or deployment.

#[path = "release_controls/behavior.rs"]
mod behavior;
#[path = "release_controls/fixture.rs"]
mod fixture;
#[path = "release_controls/rejections.rs"]
mod rejections;
