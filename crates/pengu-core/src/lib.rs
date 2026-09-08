//! PenguKit core: shared primitives for all crates.
//!
//! Everything in here is pure `no_std`-friendly style std-only code with no
//! external dependencies and no platform requirements beyond Linux-ish POSIX.

pub mod crc;
pub mod error;
pub mod io;
pub mod natives;

pub use error::{PenguError, Result};
pub use natives::find_native;

/// PenguKit version string, shared across the workspace.
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");