use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use token_ledger::{api, Ledger};

fn app() -> Router {
    api::router(Ledger::new("admin").unwrap())
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn mint_transfer_and_check_balances() {
    let app = app();
    let (s, _) = call(&app, "POST", "/mint", Some(json!({"caller":"admin","to":"alice","amount":100}))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, _) = call(&app, "POST", "/transfer", Some(json!({"from":"alice","to":"bob","amount":"30"}))).await;
    assert_eq!(s, StatusCode::CREATED);

    let (_, a) = call(&app, "GET", "/balances/alice", None).await;
    let (_, b) = call(&app, "GET", "/balances/bob", None).await;
    let (_, t) = call(&app, "GET", "/supply", None).await;
    assert_eq!(a["balance"], "70");
    assert_eq!(b["balance"], "30");
    assert_eq!(t["totalSupply"], "100");

    let (_, e) = call(&app, "GET", "/events", None).await;
    assert_eq!(e["events"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn insufficient_balance_is_422() {
    let app = app();
    call(&app, "POST", "/mint", Some(json!({"caller":"admin","to":"alice","amount":5}))).await;
    let (s, body) = call(&app, "POST", "/transfer", Some(json!({"from":"alice","to":"bob","amount":6}))).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "insufficient_balance");
}

#[tokio::test]
async fn non_owner_mint_is_forbidden() {
    let (s, body) = call(&app(), "POST", "/mint", Some(json!({"caller":"eve","to":"eve","amount":1}))).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn invalid_inputs_are_400() {
    let app = app();
    let (s, _) = call(&app, "POST", "/mint", Some(json!({"caller":"admin","to":"alice","amount":0}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(&app, "POST", "/mint", Some(json!({"caller":"admin","to":"alice","amount":"abc"}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
