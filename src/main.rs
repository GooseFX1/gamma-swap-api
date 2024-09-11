use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

mod handlers;
mod models;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/quote", get(handlers::quote))
        .route("/swap", post(handlers::swap))
        .route("/swap-instructions", post(handlers::swap_instructions))
        .layer(CorsLayer::permissive());

    println!("Gamma Swap API running on http://localhost:3000");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
