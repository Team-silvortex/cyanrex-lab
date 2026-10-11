//! Independent metadata index tests; no public route, Artifact bytes or migration acceptance.
use cyanrex_engine::{
    models::collaboration::*,
    services::task_store::{TaskContentIndexStore, TaskStoreError},
};
use sqlx_postgres::{PgPool, PgPoolOptions};

#[path = "task_content_index/defaults.rs"]
mod defaults;
use defaults::{owner, scope};
#[path = "task_content_index/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "task_content_index/index_trusted.rs"]
mod index_trusted;
