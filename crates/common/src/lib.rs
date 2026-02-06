//! Common utilities and types shared across xcprobe crates.

pub mod error;
pub mod hash;
pub mod os;
pub mod timestamp;

pub use error::{Error, Result};
pub use os::OsType;
pub use timestamp::Timestamp;
