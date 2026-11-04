# Lending – loan service (baseline)

The Lending domain's loan service for the agentic SDLC demo (Rust, axum). It stands in for the Lending integration and
core systems: quotes with APR, and the existing personal loans of the (fictional) demo customers.

- `POST /loan/quote`: monthly payment, schedule, total interest, total cost of credit, APR.
- `GET /customers/{customer_id}/loans`: the customer's existing loans (demo data in `api/src/loans.rs`).
- Run: `cd api && cargo run` (port 8080, or `PORT=…`). Test: `cargo test`.
- Contracts are owned by the architecture repository of the run (`interfaces/`); low-level designs go to `design/`.
