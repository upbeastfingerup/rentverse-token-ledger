use std::fmt;

/// Every way a ledger operation can be rejected. A rejected operation never
/// mutates state (transactions are atomic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// Account identifier is empty or malformed.
    InvalidAccount(String),
    /// Amount must be strictly greater than zero.
    ZeroAmount,
    /// Sender balance is lower than the requested transfer.
    InsufficientBalance { available: u128, required: u128 },
    /// Only the ledger owner may mint.
    Unauthorized { caller: String },
    /// Arithmetic would overflow `u128` (balance or total supply).
    Overflow,
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccount(why) => write!(f, "invalid account: {why}"),
            Self::ZeroAmount => write!(f, "amount must be greater than zero"),
            Self::InsufficientBalance { available, required } => write!(
                f,
                "insufficient balance: available {available}, required {required}"
            ),
            Self::Unauthorized { caller } => write!(f, "'{caller}' is not allowed to mint"),
            Self::Overflow => write!(f, "arithmetic overflow"),
        }
    }
}

impl std::error::Error for LedgerError {}
