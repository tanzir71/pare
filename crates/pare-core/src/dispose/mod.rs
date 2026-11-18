//! Disposal: Recycle Bin, direct delete (rebuildable data only), tool commands, and the op log.

pub mod executor;
pub mod oplog;
pub mod recycle;

pub use executor::{CleanOptions, CleanReport, clean};
pub use oplog::{OpLog, read_log};
