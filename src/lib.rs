//! MMBak File Parser
//!
//! A Rust library for parsing MoneyManager `.mmbak` backup files.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use mmbak_parser::MMBakFile;
//!
//! let backup = MMBakFile::open("backup.mmbak").unwrap();
//! println!("Transactions: {}", backup.transactions().len());
//! for tx in backup.active_transactions() {
//!     println!("{}: {} - {}", tx.date, tx.transaction_type, tx.format_amount());
//! }
//! ```

pub mod error;
pub mod types;
pub mod reader;

#[cfg(test)]
mod tests;

// Re-export main types
pub use error::{MMBakError, Result};
pub use types::*;
pub use reader::{MMBakFile, MMBakReader};
