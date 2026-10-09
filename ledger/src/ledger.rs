use std::collections::HashMap;

use serde::Serialize;

use crate::error::LedgerError;

const MAX_ACCOUNT_LEN: usize = 64;

/// Append-only record of a state change, analogous to an emitted contract event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    Mint { seq: u64, to: String, amount: u128 },
    Transfer { seq: u64, from: String, to: String, amount: u128 },
}

/// In-memory token ledger.
///
/// Invariant: the sum of all balances always equals `total_supply`.
/// Every mutating method validates first and only then applies changes, so a
/// failed call leaves the ledger untouched.
#[derive(Debug, Clone)]
pub struct Ledger {
    owner: String,
    balances: HashMap<String, u128>,
    total_supply: u128,
    events: Vec<Event>,
}

impl Ledger {
    /// Creates an empty ledger; `owner` is the only account allowed to mint.
    pub fn new(owner: impl Into<String>) -> Result<Self, LedgerError> {
        let owner = owner.into();
        validate_account(&owner)?;
        Ok(Self { owner, balances: HashMap::new(), total_supply: 0, events: Vec::new() })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn total_supply(&self) -> u128 {
        self.total_supply
    }

    /// Balance of `account`; unknown accounts have a balance of zero.
    pub fn balance_of(&self, account: &str) -> u128 {
        self.balances.get(account).copied().unwrap_or(0)
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Creates `amount` new tokens in `to`. Restricted to the owner.
    pub fn mint(&mut self, caller: &str, to: &str, amount: u128) -> Result<&Event, LedgerError> {
        if caller != self.owner {
            return Err(LedgerError::Unauthorized { caller: caller.to_owned() });
        }
        validate_account(to)?;
        validate_amount(amount)?;

        let new_balance = self.balance_of(to).checked_add(amount).ok_or(LedgerError::Overflow)?;
        let new_supply = self.total_supply.checked_add(amount).ok_or(LedgerError::Overflow)?;

        self.balances.insert(to.to_owned(), new_balance);
        self.total_supply = new_supply;
        Ok(self.record(|seq| Event::Mint { seq, to: to.to_owned(), amount }))
    }

    /// Moves `amount` tokens from `from` to `to`.
    pub fn transfer(&mut self, from: &str, to: &str, amount: u128) -> Result<&Event, LedgerError> {
        validate_account(from)?;
        validate_account(to)?;
        validate_amount(amount)?;

        let available = self.balance_of(from);
        let remaining = available
            .checked_sub(amount)
            .ok_or(LedgerError::InsufficientBalance { available, required: amount })?;

        if from == to {
            // Self-transfer: balance was verified above, nothing moves.
            return Ok(self.record(|seq| Event::Transfer { seq, from: from.to_owned(), to: to.to_owned(), amount }));
        }

        // Cannot overflow in practice (bounded by total_supply) but stay checked.
        let credited = self.balance_of(to).checked_add(amount).ok_or(LedgerError::Overflow)?;

        self.balances.insert(from.to_owned(), remaining);
        self.balances.insert(to.to_owned(), credited);
        Ok(self.record(|seq| Event::Transfer { seq, from: from.to_owned(), to: to.to_owned(), amount }))
    }

    fn record(&mut self, build: impl FnOnce(u64) -> Event) -> &Event {
        let seq = self.events.len() as u64 + 1;
        self.events.push(build(seq));
        self.events.last().expect("just pushed")
    }
}

fn validate_account(account: &str) -> Result<(), LedgerError> {
    if account.trim().is_empty() {
        return Err(LedgerError::InvalidAccount("must not be empty".into()));
    }
    if account.len() > MAX_ACCOUNT_LEN {
        return Err(LedgerError::InvalidAccount(format!("longer than {MAX_ACCOUNT_LEN} characters")));
    }
    Ok(())
}

fn validate_amount(amount: u128) -> Result<(), LedgerError> {
    if amount == 0 {
        Err(LedgerError::ZeroAmount)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> Ledger {
        Ledger::new("admin").unwrap()
    }

    #[test]
    fn mint_increases_balance_and_supply() {
        let mut l = ledger();
        l.mint("admin", "alice", 100).unwrap();
        l.mint("admin", "alice", 50).unwrap();
        assert_eq!(l.balance_of("alice"), 150);
        assert_eq!(l.total_supply(), 150);
    }

    #[test]
    fn mint_requires_owner() {
        let mut l = ledger();
        let err = l.mint("mallory", "mallory", 1).unwrap_err();
        assert_eq!(err, LedgerError::Unauthorized { caller: "mallory".into() });
        assert_eq!(l.total_supply(), 0);
    }

    #[test]
    fn mint_rejects_zero_and_bad_account() {
        let mut l = ledger();
        assert_eq!(l.mint("admin", "alice", 0).unwrap_err(), LedgerError::ZeroAmount);
        assert!(matches!(l.mint("admin", " ", 5), Err(LedgerError::InvalidAccount(_))));
    }

    #[test]
    fn mint_overflow_is_rejected_atomically() {
        let mut l = ledger();
        l.mint("admin", "alice", u128::MAX).unwrap();
        assert_eq!(l.mint("admin", "bob", 1).unwrap_err(), LedgerError::Overflow);
        assert_eq!(l.balance_of("bob"), 0);
        assert_eq!(l.total_supply(), u128::MAX);
    }

    #[test]
    fn transfer_moves_funds_and_preserves_supply() {
        let mut l = ledger();
        l.mint("admin", "alice", 100).unwrap();
        l.transfer("alice", "bob", 40).unwrap();
        assert_eq!(l.balance_of("alice"), 60);
        assert_eq!(l.balance_of("bob"), 40);
        assert_eq!(l.total_supply(), 100);
    }

    #[test]
    fn transfer_with_insufficient_balance_fails_without_side_effects() {
        let mut l = ledger();
        l.mint("admin", "alice", 10).unwrap();
        let err = l.transfer("alice", "bob", 11).unwrap_err();
        assert_eq!(err, LedgerError::InsufficientBalance { available: 10, required: 11 });
        assert_eq!(l.balance_of("alice"), 10);
        assert_eq!(l.balance_of("bob"), 0);
        assert_eq!(l.events().len(), 1);
    }

    #[test]
    fn transfer_from_unknown_account_fails() {
        let mut l = ledger();
        assert!(matches!(l.transfer("ghost", "bob", 1), Err(LedgerError::InsufficientBalance { available: 0, .. })));
    }

    #[test]
    fn transfer_rejects_zero_amount() {
        let mut l = ledger();
        l.mint("admin", "alice", 10).unwrap();
        assert_eq!(l.transfer("alice", "bob", 0).unwrap_err(), LedgerError::ZeroAmount);
    }

    #[test]
    fn self_transfer_keeps_balance() {
        let mut l = ledger();
        l.mint("admin", "alice", 10).unwrap();
        l.transfer("alice", "alice", 10).unwrap();
        assert_eq!(l.balance_of("alice"), 10);
        assert!(l.transfer("alice", "alice", 11).is_err());
    }

    #[test]
    fn unknown_account_has_zero_balance() {
        assert_eq!(ledger().balance_of("nobody"), 0);
    }

    #[test]
    fn events_are_sequenced() {
        let mut l = ledger();
        l.mint("admin", "alice", 5).unwrap();
        l.transfer("alice", "bob", 5).unwrap();
        let seqs: Vec<u64> = l
            .events()
            .iter()
            .map(|e| match e {
                Event::Mint { seq, .. } | Event::Transfer { seq, .. } => *seq,
            })
            .collect();
        assert_eq!(seqs, vec![1, 2]);
    }
}
