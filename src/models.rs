use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct QuoteParams {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: String,
    pub slippage_bps: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct QuoteResponse {
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub other_amount_threshold: String,
    pub swap_mode: SwapMode,
    pub slippage_bps: u32,
    pub platform_fee: Option<PlatformFee>,
    pub price_impact_pct: String,
    pub route_plan: Vec<RoutePlanStep>,
    pub context_slot: Option<u64>,
    pub time_taken: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub enum SwapMode {
    ExactIn,
    ExactOut,
}

#[derive(Serialize, Deserialize)]
pub struct PlatformFee {
    pub amount: String,
    pub fee_bps: u32,
}

#[derive(Serialize, Deserialize)]
pub struct RoutePlanStep {
    pub swap_info: SwapInfo,
    pub percent: u32,
}

#[derive(Serialize, Deserialize)]
pub struct SwapInfo {
    pub amm_key: String,
    pub label: Option<String>,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_amount: String,
    pub fee_mint: String,
}

#[derive(Deserialize)]
pub struct SwapRequest {
    pub user_public_key: String,
    pub quote_response: QuoteResponse,
}

#[derive(Serialize)]
pub struct SwapResponse {
    pub swap_transaction: String,
    pub last_valid_block_height: u64,
    pub prioritization_fee_lamports: Option<u64>,
}

#[derive(Serialize)]
pub struct SwapInstructionsResponse {
    pub token_ledger_instruction: Option<Instruction>,
    pub compute_budget_instructions: Vec<Instruction>,
    pub setup_instructions: Vec<Instruction>,
    pub swap_instruction: Instruction,
    pub cleanup_instruction: Option<Instruction>,
    pub address_lookup_table_addresses: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct Instruction {
    pub program_id: String,
    pub accounts: Vec<AccountMeta>,
    pub data: String,
}

#[derive(Serialize)]
pub struct AccountMeta {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}
