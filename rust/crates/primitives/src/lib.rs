//! Internal secure primitives shared by the transactional core and host agent.
pub mod compat;
pub mod error;
pub mod file_set;
pub mod index_store;
pub mod inventory;
pub mod items;
pub mod runtime;
pub mod storage;
pub mod store;
pub use error::{Error, Result};
pub mod reference;
