//! HTTP adapter. Handlers only translate JSON <-> domain calls; all rules live
//! in [`crate::ledger`].

use std::sync::{Arc, Mutex, MutexGuard};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

use crate::{error::LedgerError, ledger::Ledger};

pub type SharedLedger = Arc<Mutex<Ledger>>;

pub fn router(ledger: Ledger) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
        .route("/mint", post(mint))
        .route("/transfer", post(transfer))
        .route("/balances/:account", get(balance))
        .route("/supply", get(supply))
        .route("/events", get(events))
        .with_state(Arc::new(Mutex::new(ledger)))
}

// Amounts are u128; JSON numbers can't represent that safely, so clients may
// send either a number or a decimal string.
#[derive(Deserialize)]
#[serde(untagged)]
enum Amount {
    Num(u64),
    Str(String),
}

impl Amount {
    fn parse(self) -> Result<u128, ApiError> {
        match self {
            Amount::Num(n) => Ok(n as u128),
            Amount::Str(s) => s
                .trim()
                .parse()
                .map_err(|_| ApiError::BadRequest("amount must be a non-negative integer".into())),
        }
    }
}

#[derive(Deserialize)]
struct MintRequest {
    caller: String,
    to: String,
    amount: Amount,
}

#[derive(Deserialize)]
struct TransferRequest {
    from: String,
    to: String,
    amount: Amount,
}

async fn mint(State(s): State<SharedLedger>, Json(req): Json<MintRequest>) -> Result<Response, ApiError> {
    let amount = req.amount.parse()?;
    let mut ledger = lock(&s);
    let event = ledger.mint(&req.caller, &req.to, amount)?.clone();
    let balance = ledger.balance_of(&req.to);
    Ok((StatusCode::CREATED, Json(json!({ "event": event, "balance": balance.to_string() }))).into_response())
}

async fn transfer(State(s): State<SharedLedger>, Json(req): Json<TransferRequest>) -> Result<Response, ApiError> {
    let amount = req.amount.parse()?;
    let mut ledger = lock(&s);
    let event = ledger.transfer(&req.from, &req.to, amount)?.clone();
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "event": event,
            "balances": {
                req.from.clone(): ledger.balance_of(&req.from).to_string(),
                req.to.clone(): ledger.balance_of(&req.to).to_string(),
            }
        })),
    )
        .into_response())
}

async fn balance(State(s): State<SharedLedger>, Path(account): Path<String>) -> Json<serde_json::Value> {
    let balance = lock(&s).balance_of(&account);
    Json(json!({ "account": account, "balance": balance.to_string() }))
}

async fn supply(State(s): State<SharedLedger>) -> Json<serde_json::Value> {
    Json(json!({ "totalSupply": lock(&s).total_supply().to_string() }))
}

async fn events(State(s): State<SharedLedger>) -> Json<serde_json::Value> {
    Json(json!({ "events": lock(&s).events() }))
}

// A poisoned lock only means another handler panicked; ledger methods validate
// before mutating, so the data is still consistent.
fn lock(s: &SharedLedger) -> MutexGuard<'_, Ledger> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

enum ApiError {
    Ledger(LedgerError),
    BadRequest(String),
}

impl From<LedgerError> for ApiError {
    fn from(e: LedgerError) -> Self {
        Self::Ledger(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            Self::Ledger(e) => {
                let (status, code) = match &e {
                    LedgerError::InvalidAccount(_) => (StatusCode::BAD_REQUEST, "invalid_account"),
                    LedgerError::ZeroAmount => (StatusCode::BAD_REQUEST, "zero_amount"),
                    LedgerError::Overflow => (StatusCode::BAD_REQUEST, "overflow"),
                    LedgerError::InsufficientBalance { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "insufficient_balance"),
                    LedgerError::Unauthorized { .. } => (StatusCode::FORBIDDEN, "unauthorized"),
                };
                (status, code, e.to_string())
            }
        };
        (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
    }
}
