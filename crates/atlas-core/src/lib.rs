//! Shared domain contracts. Native commands and coordinator transports use the
//! same definitions; neither this crate nor a DTO grants authenticated access.
pub mod contracts;
pub mod inventory;
pub mod operations;
pub mod store;
pub mod validation;

pub use contracts::*;
pub use store::{MigrationSummary, Store, migrate_legacy, restore_legacy};
pub use validation::{coordinate, hostname, observation_time, text};
