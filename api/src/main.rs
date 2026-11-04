#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    // 0.0.0.0 so the iOS Simulator and an iPhone on the same network can reach it.
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("lending loan-api listening on http://{addr}");
    axum::serve(listener, loan_api::app()).await.expect("serve");
}
