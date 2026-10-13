//! Service ownership index. Groups are navigation, not new namespaces or process isolation.
//! `state::build_state` and `application::build_router` decide actual runtime composition.
//! In particular, auth_service contains BOTH live AuthService and explicit durable_source commands;
//! the latter remain unmounted preparation. A declaration here grants no authority or execution.

// Group: Live instance services
pub mod ai_agent_settings;
pub mod auth_service;
pub mod classroom;
pub mod command_dispatcher;
pub mod environment_checker;
pub mod module_manager;

// Group: Teaching and local execution
pub mod c_header_module;
pub mod ebpf_loader;
pub mod learning_catalog;
pub mod learning_store;
pub mod runner_driver;
pub mod runner_manager;
pub mod script_store;

// Group: Signed Runner Agent services
pub mod runner_agent_authenticator;
pub mod runner_agent_client;
pub mod runner_agent_executor;
pub mod runner_agent_registry;
pub mod runner_job_queue;

// Group: Event telemetry
pub mod event_bus;
mod event_bus_codec;
mod event_bus_db;
mod event_bus_db_config;
mod event_bus_db_parser;
mod event_bus_filter;
mod event_bus_policy;

// Group: Prepared identity and SQL support
// legacy_workspace is a pure permission preview, never live authorization or data migration.
// prepared_resource_sql shares storage mechanics, not Session policy or permission decisions.
pub mod collaboration_identity_store;
pub mod legacy_workspace;
mod prepared_resource_sql;

// Group: Prepared Task and content
// Pure definitions/plans and explicit stores coexist here; listing a store installs nothing.
#[cfg(unix)]
pub mod artifact_store;
#[cfg(unix)]
pub mod draft_intent_journal;
pub mod review_store;
pub mod task_catalog;
pub mod task_content;
pub mod task_draft_import;
pub mod task_store;

// Group: Prepared file distribution
// Manifests, byte checks, ledgers and clients are not interchangeable authority or lifetime gates.
#[cfg(unix)]
pub mod file_chunk_client;
#[cfg(unix)]
pub mod file_chunk_receiver;
pub mod file_publisher_grant_store;
pub mod file_recipient_grant_store;
pub mod file_transfer;
#[cfg(unix)]
pub mod file_upload_chunk_journal;
#[cfg(unix)]
pub mod file_upload_publication;
#[cfg(unix)]
pub mod file_upload_staging;
pub mod file_upload_store;
