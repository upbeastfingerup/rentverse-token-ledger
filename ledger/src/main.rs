use std::net::SocketAddr;

use token_ledger::{api, Ledger};

#[tokio::main]
async fn main() {
    let owner = std::env::var("LEDGER_OWNER").unwrap_or_else(|_| "admin".into());
    let port: u16 = std::env::var("LEDGER_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    let ledger = Ledger::new(owner).expect("LEDGER_OWNER must be a valid account id");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("ledger-api (owner '{}') listening on http://{addr}", ledger.owner());

    axum::Server::bind(&addr)
        .serve(api::router(ledger).into_make_service())
        .await
        .expect("server error");
}
