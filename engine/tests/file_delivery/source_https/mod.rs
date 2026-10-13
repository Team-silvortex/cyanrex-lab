//! Combined actual Source, original files and native HTTPS proof; not a deployed listener.
use super::*;

#[path = "../../file_delivery_tls/certificates.rs"]
mod certificates;
use certificates::Certificates;
mod native_io;
mod request;
mod server;
use server::{Hold, HoldAt, Kind, SourceTlsServer, TlsObservation};
mod acceptance;
mod cancellation;
mod fixture;
mod revocation;
mod support;
