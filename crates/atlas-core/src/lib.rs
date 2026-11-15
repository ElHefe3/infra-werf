//! Shared domain contracts. Native commands and coordinator transports use the
//! same definitions; neither this crate nor a DTO grants authenticated access.
pub mod contracts;
pub mod extensions;
pub mod files;
pub mod health;
pub mod inventory;
pub mod monitoring;
pub mod operations;
pub mod plugin;
mod sftp;
pub mod ssh;
pub mod store;
pub mod terminal;
pub mod tunnel;
pub mod validation;
pub mod workloads;

pub use contracts::*;
pub use store::{MigrationSummary, Store, migrate_legacy, restore_legacy};
pub use validation::{coordinate, hostname, observation_time, text};
