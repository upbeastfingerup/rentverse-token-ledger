# Token Ledger (Rust)

A minimal ERC-20-style token ledger, simulated in memory (no blockchain/network), exposed over HTTP and
proxied by the Express server at `/api/ledger/*`.

## Architecture
| File | Role |
|---|---|
| `src/ledger.rs` | Pure domain logic: balances, mint, transfer, append-only event log. No I/O or framework deps. |
| `src/error.rs` | `LedgerError` – typed failure modes. |
| `src/api.rs` | axum adapter: JSON in/out, maps `LedgerError` to HTTP status codes. |
| `tests/api.rs` | End-to-end HTTP tests (in-process, no socket). |

Blockchain-style properties: owner-gated `mint` (like `onlyOwner`), checked `u128` arithmetic (no overflow/underflow),
atomic state changes (validate, then mutate), conserved supply (sum of balances == total supply), and a sequenced
event log (like emitted contract events).

## Run
```bash
npm run ledger         # starts on 127.0.0.1:8080 (LEDGER_PORT, LEDGER_OWNER=admin)
npm run ledger:test    # unit + API tests
node server/server.js  # Express on :3099 proxies /api/ledger/* (LEDGER_API_URL to override)
```

## Endpoints (`/api/ledger/...`)
| Method | Path | Body | Notes |
|---|---|---|---|
| POST | `/mint` | `{caller,to,amount}` | 201; 403 if caller isn't owner |
| POST | `/transfer` | `{from,to,amount}` | 201; 422 if insufficient balance |
| GET | `/balances/:account` | | unknown accounts return `"0"` |
| GET | `/supply` | | |
| GET | `/events` | | ordered mint/transfer history |

Amounts may be JSON numbers or decimal strings (balances are returned as strings, since `u128` exceeds JS safe integers).
Errors: `{"error":{"code","message"}}`.

```bash
curl -XPOST localhost:3099/api/ledger/mint -H 'content-type: application/json' -d '{"caller":"admin","to":"alice","amount":100}'
curl -XPOST localhost:3099/api/ledger/transfer -H 'content-type: application/json' -d '{"from":"alice","to":"bob","amount":30}'
curl localhost:3099/api/ledger/balances/bob
```

State is in-memory and resets on restart. Note: `Cargo.lock` pins some crates for Rust 1.70 compatibility.
