use crate::gfx_swap::quote::QuoteError;
use crate::gfx_swap::GfxSwapClient;
use crate::handlers::{ApiResponse, ErrorResponse};

use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
};
use log::error;
use swap_api::quote::{QuoteRequest, QuoteResponse};

pub async fn quote(
    State(gfx_swap): State<GfxSwapClient>,
    Query(params): Query<QuoteRequest>,
) -> (StatusCode, Json<ApiResponse<QuoteResponse>>) {
    match gfx_swap.quote(&params).await {
        Ok(quote) => (StatusCode::OK, Json(ApiResponse::T(quote))),
        Err(QuoteError::InvalidRequest(message)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::Error(ErrorResponse { message })),
        ),
        Err(e) => {
            error!("Error getting quote: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::Error(ErrorResponse {
                    message: e.to_string(),
                })),
            )
        }
    }
}
