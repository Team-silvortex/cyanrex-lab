//! Run the existing transport, body and admission contract unchanged against the explicit
//! catalogue adapter. The only adaptation is trusted configuration and its create reference.
use super::*;
use cyanrex_engine::platform_http::{
    build_catalog_task_content_router as build_task_content_router,
    CatalogTaskContentHttpState as TaskContentHttpState,
};

fn create_body(task: TaskId, value: &TaskContentManifest) -> String {
    fixture::create_body(task, &values()[0].reference, value)
}

#[path = "../platform_http/admission.rs"]
mod admission;
#[path = "../platform_http/transport.rs"]
mod transport;
