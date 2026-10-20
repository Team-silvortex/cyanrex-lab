#[cfg(test)]
extern crate self as cyanrex_engine;

pub mod application;
pub mod config;
pub mod domain_packs;
mod metrics;
pub mod models;
#[cfg(unix)]
pub mod platform_host;
#[cfg(unix)]
pub mod platform_http;
#[cfg(unix)]
mod platform_origin;
pub mod routes;
pub mod services;
mod sqlx_compat;
pub mod state;

pub use application::{build_allowed_origins, build_router};
pub use state::{build_state, AppState};
