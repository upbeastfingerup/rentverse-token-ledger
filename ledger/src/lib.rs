//! A small token ledger that behaves like a minimal ERC-20-style contract,
//! without any live blockchain behind it.
//!
//! * [`ledger`] – pure domain logic (no I/O, no framework dependencies).
//! * [`error`]  – the typed failure modes of ledger operations.
//! * [`api`]    – thin HTTP adapter (axum) over the domain logic.

pub mod api;
pub mod error;
pub mod ledger;

pub use error::LedgerError;
pub use ledger::{Event, Ledger};
