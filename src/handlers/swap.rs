use crate::gfx_swap::swap::SwapError;
use crate::gfx_swap::GfxSwapClient;
use crate::handlers::{ApiResponse, ErrorResponse};

use axum::{
    extract::{Json, State},
    http::StatusCode,
};
use jupiter_swap_api_client::swap::{SwapInstructionsResponseInternal, SwapRequest, SwapResponse};
use log::error;

pub async fn swap_instructions(
    State(gfx_swap): State<GfxSwapClient>,
    Json(params): Json<SwapRequest>,
) -> (
    StatusCode,
    Json<ApiResponse<SwapInstructionsResponseInternal>>,
) {
    match gfx_swap.swap_instructions(&params).await {
        Ok(quote) => (StatusCode::OK, Json(ApiResponse::T(quote))),
        Err(SwapError::InvalidRequest(message)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::Error(ErrorResponse { message })),
        ),
        Err(e) => {
            error!("swap-instructions error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::Error(ErrorResponse {
                    message: e.to_string(),
                })),
            )
        }
    }
}

pub async fn swap_transaction(
    State(gfx_swap): State<GfxSwapClient>,
    Json(params): Json<SwapRequest>,
) -> (StatusCode, Json<ApiResponse<SwapResponse>>) {
    match gfx_swap.swap_transaction(&params).await {
        Ok(quote) => (StatusCode::OK, Json(ApiResponse::T(quote))),
        Err(SwapError::InvalidRequest(message)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::Error(ErrorResponse { message })),
        ),
        Err(e) => {
            error!("swap-transaction error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::Error(ErrorResponse {
                    message: e.to_string(),
                })),
            )
        }
    }
}
