//! Typed transactional application and explicit compatibility/integration boundaries.
pub mod activity;
pub mod auth;
pub mod authoring;
mod casefold;
pub mod catalog;
pub mod compat;
pub mod composition;
pub mod contracts;
pub mod diagnostics;
pub mod error;
pub mod execution;
pub mod file_set;
pub mod graph;
pub mod history;
pub mod http;
pub mod index_store;
pub mod inventory;
pub mod items;
pub mod jobs;
pub mod log_index;
pub mod maintenance;
pub mod media;
pub mod onboarding;
pub mod progress;
pub mod query;
pub mod reconcile;
pub mod results;
pub mod runtime;
pub mod scenario;
pub mod software;
pub mod storage;
pub mod store;
pub use error::{Error, Result};
pub use store::{Application, Config};

#[cfg(test)]
#[path = "../../../../tests/rust/application.rs"]
mod tests;
