#![cfg(unix)]
//! Explicit two-route composition only; a closed Source does not manufacture file authority.
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_file_distribution_router, FileDistributionHttpConfigError, FileDistributionHttpState,
    },
    services::{
        auth_service::durable_source::{DurableAuthSource, SessionFileChunkReadWorkspace},
        file_transfer::staging::ChunkStaging,
        file_upload_publication::FileUploadPublicationStore,
    },
};
use http_body_util::BodyExt;
use sqlx_postgres::{PgPool, PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "file_distribution_http/admission.rs"]
mod admission;
#[path = "file_distribution_http/baseline.rs"]
mod baseline;
#[path = "file_distribution_http/bodies.rs"]
mod bodies;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_distribution_http/configuration.rs"]
mod configuration;
#[path = "file_distribution_http/contract.rs"]
mod contract;
#[path = "file_distribution_http/fixture.rs"]
mod fixture;
#[path = "file_distribution_http/lifetime.rs"]
mod lifetime;
#[path = "file_distribution_http/precedence.rs"]
mod precedence;
#[path = "file_distribution_http/support.rs"]
mod support;
