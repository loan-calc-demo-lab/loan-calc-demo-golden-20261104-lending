pub mod calc;
pub mod loans;

use axum::{extract::{Path, State}, http::StatusCode, response::IntoResponse, routing::{get, post}, Json, Router};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Shared state of the service: the loan book (in memory in this demo).
#[derive(Clone)]
pub struct AppState {
    pub loans: Arc<Mutex<loans::LoanBook>>,
}

impl AppState {
    pub fn demo() -> Self {
        AppState { loans: Arc::new(Mutex::new(loans::LoanBook::demo())) }
    }
}

/// The service with fresh demo data.
pub fn app() -> Router {
    app_with(AppState::demo())
}

pub fn app_with(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/loan/quote", post(quote_handler))
        .route("/customers/:customer_id/loans", get(customer_loans))
        .with_state(state)
}

async fn quote_handler(Json(req): Json<calc::QuoteRequest>) -> impl IntoResponse {
    match calc::quote(&req) {
        Ok(q) => (StatusCode::OK, Json(serde_json::to_value(q).unwrap())),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({"error": "validation", "field": e.field, "message": e.message}))),
    }
}

/// The customer's existing loans (empty list for an unknown customer).
async fn customer_loans(State(state): State<AppState>, Path(customer_id): Path<String>) -> impl IntoResponse {
    let loans = state.loans.lock().unwrap().for_customer(&customer_id);
    Json(json!({ "customer_id": customer_id, "loans": loans }))
}
