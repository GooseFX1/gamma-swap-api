use crate::models::*;
use axum::{extract::Query, Json};

pub async fn quote(Query(params): Query<QuoteParams>) -> Json<QuoteResponse> {
    // Mock
    Json(QuoteResponse {
        input_mint: params.input_mint.clone(),
        output_mint: params.output_mint.clone(),
        in_amount: params.amount.clone(),
        out_amount: (params.amount.parse::<f64>().unwrap() * 0.98).to_string(),
        other_amount_threshold: "0".to_string(),
        swap_mode: SwapMode::ExactIn,
        slippage_bps: params.slippage_bps.unwrap_or(50),
        platform_fee: Some(PlatformFee {
            amount: "10".to_string(),
            fee_bps: 10,
        }),
        price_impact_pct: "0.1".to_string(),
        route_plan: vec![RoutePlanStep {
            swap_info: SwapInfo {
                amm_key: "GaMMAt2scxuGJu3esLfLsJZaC482MPEKswdx8DfUsHCR".to_string(),
                label: Some("Gamma".to_string()),
                input_mint: params.input_mint.clone(),
                output_mint: params.output_mint.clone(),
                in_amount: params.amount.clone(),
                out_amount: (params.amount.parse::<f64>().unwrap() * 0.98).to_string(),
                fee_amount: "2".to_string(),
                fee_mint: params.input_mint,
            },
            percent: 100,
        }],
        context_slot: Some(1000000),
        time_taken: Some(0.05),
    })
}

pub async fn swap(Json(_request): Json<SwapRequest>) -> Json<SwapResponse> {
    // Mock implementation
    Json(SwapResponse {
        swap_transaction: "mock_transaction_data".to_string(),
        last_valid_block_height: 1000000,
        prioritization_fee_lamports: Some(5000),
    })
}

pub async fn swap_instructions(Json(request): Json<SwapRequest>) -> Json<SwapInstructionsResponse> {
    // Mock implementation
    Json(SwapInstructionsResponse {
        token_ledger_instruction: Some(Instruction {
            program_id: "TokenLedger111111111111111111111111111111111".to_string(),
            accounts: vec![],
            data: "mock_token_ledger_data".to_string(),
        }),
        compute_budget_instructions: vec![Instruction {
            program_id: "ComputeBudget111111111111111111111111111111".to_string(),
            accounts: vec![],
            data: "mock_compute_budget_data".to_string(),
        }],
        setup_instructions: vec![Instruction {
            program_id: "Setup111111111111111111111111111111111111".to_string(),
            accounts: vec![],
            data: "mock_setup_data".to_string(),
        }],
        swap_instruction: Instruction {
            program_id: "GaMMAt2scxuGJu3esLfLsJZaC482MPEKswdx8DfUsHCR".to_string(),
            accounts: vec![AccountMeta {
                pubkey: request.user_public_key,
                is_signer: true,
                is_writable: true,
            }],
            data: "mock_swap_data".to_string(),
        },
        cleanup_instruction: Some(Instruction {
            program_id: "Cleanup11111111111111111111111111111111111".to_string(),
            accounts: vec![],
            data: "mock_cleanup_data".to_string(),
        }),
        address_lookup_table_addresses: vec![
            "AddressLookupTable111111111111111111111111111".to_string()
        ],
    })
}
