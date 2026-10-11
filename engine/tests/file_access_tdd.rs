//! Pure claimed grant metadata and exact candidate matching, never current download authority.
use cyanrex_engine::models::collaboration::*;
use serde_json::{json, Value};
use uuid::Uuid;

#[path = "file_access/behavior.rs"]
mod behavior;
#[path = "file_access/fixture.rs"]
mod fixture;
#[path = "file_access/grants.rs"]
mod grants;
#[path = "file_access/json_shape.rs"]
mod json_shape;
#[path = "file_access/json_support.rs"]
mod json_support;
#[path = "file_access/json_values.rs"]
mod json_values;
#[path = "file_access/references.rs"]
mod references;
