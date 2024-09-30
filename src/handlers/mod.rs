pub mod quote;
pub mod swap;

use serde::Serialize;

#[derive(Serialize)]
pub enum ApiResponse<T> {
    T(T),
    Error(ErrorResponse),
}

#[derive(Serialize)]
pub struct ErrorResponse {
    message: String,
}
