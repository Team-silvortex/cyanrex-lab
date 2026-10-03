//! C-M1 contracts for the next collaboration core; not an HTTP API or a storage migration.
//!
//! Parsing validates structure, not ownership, authentication or reference existence. No ID is
//! allocated on a read. The future durable registry must allocate and persist mappings exactly once.
//! IDs are distinct Rust types even though their JSON representation is a UUID string:
//!
//! ```compile_fail
//! use cyanrex_engine::models::collaboration::{PrincipalId, WorkspaceId};
//! let workspace: WorkspaceId = "8b075b0b-8118-4c37-9224-337ba22f40c6".parse().unwrap();
//! let principal: PrincipalId = workspace;
//! ```

mod artifact;
mod content;
mod event;
mod identity;
mod references;
mod review;
mod scalars;
mod task;
mod work;

pub use artifact::*;
pub use content::*;
pub use event::*;
pub use identity::*;
pub use references::*;
pub use review::*;
pub use scalars::*;
pub use task::*;
pub use work::*;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ContractError(pub(crate) &'static str);
