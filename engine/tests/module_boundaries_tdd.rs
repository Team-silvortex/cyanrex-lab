//! Real HTTP router/service/filesystem boundaries with synthetic identities and a non-kernel driver.
#[path = "module_boundaries/event_mutations.rs"]
mod event_mutations;
#[path = "module_boundaries/event_reads.rs"]
mod event_reads;
#[path = "module_boundaries/event_settings.rs"]
mod event_settings;
#[path = "module_boundaries/events.rs"]
mod events;
#[path = "module_boundaries/learning_flow.rs"]
mod learning_flow;
#[path = "module_boundaries/learning_persistence.rs"]
mod learning_persistence;
#[path = "module_boundaries/learning_reads.rs"]
mod learning_reads;

#[path = "module_boundaries/script_postgres.rs"]
mod script_postgres;
#[path = "module_boundaries/storage.rs"]
mod storage;
#[path = "module_boundaries/support.rs"]
mod support;
